//! In-process contract tests: every route through the gateway router against
//! one mock upstream that stands in for all five sources. Database-backed
//! tests create a scratch database on the cluster `CATALYST_TEST_DATABASE_URL`
//! points at (schema.sql applied, dropped on teardown) and skip without it.

use std::collections::{BTreeSet, HashMap};
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
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
    /// The `events` application's document (`events.json`), the one
    /// `events::foundation` reads.
    events_flags: Mutex<Value>,
    entities: Mutex<Vec<Value>>,
    profiles: Mutex<Vec<Value>>,
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
        events_flags: Mutex::new(json!({
            "flags": {"events-decentraland-foundation-addresses": true},
            "variants": {"events-decentraland-foundation-addresses": {
                "name": "addresses", "enabled": true,
                "payload": {"type": "json", "value": "[\"0xF0000000000000000000000000000000000000F0\"]"}
            }}
        })),
        entities: Mutex::new(vec![
            entity("bafyscene", &["0,0", "0,1"]),
            entity("bafyother", &["5,5"]),
        ]),
        profiles: Mutex::new(vec![
            json!({"timestamp": "1790276949479", "avatars": [{"name": "Ryumi", "ethAddress": "0xC9C29AB98E6BC42015985165A11153F564E9F8C2", "userId": "0xc9c29ab98e6bc42015985165a11153f564e9f8c2"}]}),
        ]),
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
    async fn events_flags(AxState(m): AxState<Arc<Mock>>, headers: HeaderMap) -> Response {
        m.hit("events-flags");
        if headers.get(header::REFERER).and_then(|v| v.to_str().ok())
            != Some("https://decentraland.org")
        {
            return (StatusCode::OK, Json(json!({"flags": {}, "variants": {}}))).into_response();
        }
        (
            StatusCode::OK,
            [(header::ETAG, "\"events-flags-1\"")],
            Json(m.events_flags.lock().clone()),
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
    async fn versions(
        AxState(m): AxState<Arc<Mock>>,
        Query(q): Query<HashMap<String, String>>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        m.hit("entities/versions");
        m.bodies.lock().push(json!({"body":body,"query":q}));
        Json(json!([]))
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
        .route("/events.json", get(events_flags))
        .route("/entities/active", post(entities))
        .route("/entities/versions", post(versions))
        .route("/profiles", post(profiles))
        .route("/profiles/metadata", post(profiles_metadata))
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
    node_on_pool(settings, lazy_pool()).await
}

/// A node over `pool` without `State::new`'s mirror loads: the routes that
/// keep their own rows (the auth-api relay) on a scratch database.
async fn node_on_pool(settings: Settings, pool: PgPool) -> Node {
    let (content, tmp) = content_state(pool.clone()).await;
    let state = Arc::new(State {
        settings,
        pool: pool.clone(),
        http: crate::http::Outbound::for_tests(),
        content,
        denylist: Default::default(),
        flags: std::sync::Arc::new(feature_flags::Snapshot::from_document(
            feature_flags::embedded_document(),
            false,
        )),
        map: map::MapState::new(),
        ab_cdn: ab_cdn::Cache::new(tmp.0.join("ab-cdn"), u64::MAX).unwrap(),
        satellite: satellite::SatelliteState::new(1 << 20, 64),
        world_manifest: world_manifest::WorldManifestState::default(),
        posters: posters::PosterState::default(),
        opensea: opensea::Runtime::default(),
        runtime_config: runtime_config::RuntimeConfigState::default(),
        onboarding: Arc::new(onboarding::OnboardingState::new(pool)),
        registry_denylist: Default::default(),
        eip1654: std::sync::OnceLock::new(),
        contract_checks: Default::default(),
        auth_sockets: Default::default(),
        land_authz: std::sync::OnceLock::new(),
        land_market: std::sync::OnceLock::new(),
    });
    Node { state, _tmp: tmp }
}

/// The state alone, its temporary storage leaked for the test's lifetime.
pub(crate) async fn node_without_db_state(settings: Settings) -> Shared {
    node_on_pool_state(settings, lazy_pool()).await
}

/// [`node_on_pool`]'s state alone, its temporary storage leaked.
pub(crate) async fn node_on_pool_state(settings: Settings, pool: PgPool) -> Shared {
    let node = node_on_pool(settings, pool).await;
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

/// [`node_with_db`]'s state alone, its temporary storage leaked for the
/// test's lifetime.
pub(crate) async fn node_with_db_state(settings: Settings, pool: PgPool) -> Shared {
    let node = node_with_db(settings, pool).await;
    let state = node.state.clone();
    std::mem::forget(node);
    state
}

pub(crate) struct ScratchDb {
    pub(crate) pool: PgPool,
    admin: PgPool,
    name: String,
}

pub(crate) async fn scratch_database() -> Option<ScratchDb> {
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
    crate::db::scratch::create_database_with_schema(&admin, &name).await;
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(base.database(&name).options([("TimeZone", "UTC")]))
        .await
        .unwrap();
    Some(ScratchDb { pool, admin, name })
}

impl ScratchDb {
    /// Another pool on the same database, as a second replica holds.
    pub(crate) async fn second_pool(&self) -> PgPool {
        let url = std::env::var("CATALYST_TEST_DATABASE_URL").unwrap();
        let base: PgConnectOptions = url.parse().unwrap();
        PgPoolOptions::new()
            .max_connections(6)
            .connect_with(base.database(&self.name).options([("TimeZone", "UTC")]))
            .await
            .unwrap()
    }

    pub(crate) async fn teardown(self) {
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
/// A replica that never runs the leased mirror pass still follows the
/// shared denylist tables through its own periodic reload.
#[tokio::test]
async fn every_replica_reloads_the_shared_denylist() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let replica = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    let wallet = "0x00000000000000000000000000000000000000fe";
    assert!(!replica.state.denylist.contains(wallet));
    let jobs = Jobs::new();
    denylist::spawn_reload(
        &replica.state,
        &jobs,
        std::time::Duration::from_millis(20),
        denylist::RELOAD_TIMEOUT,
    );
    denylist::set_local(&db.pool, wallet, true).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !replica.state.denylist.contains(wallet) {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the replica reloads the shared overlay");
    denylist::set_local(&db.pool, wallet, false).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while replica.state.denylist.contains(wallet) {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("and its removal");
    jobs.shutdown(std::time::Duration::from_secs(2)).await;
    db.teardown().await;
}

#[tokio::test]
async fn denylist_reload_timeout_retains_last_snapshot_and_recovers_after_sql_unblocks() {
    let Some(db) = scratch_database().await else {
        return;
    };
    denylist::replace(
        &db.pool,
        &[denylist::UserEntry {
            wallet: "0xPrevious".into(),
        }],
    )
    .await
    .unwrap();
    let replica = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    let previous = replica.state.denylist.list();
    let app = router(replica.state.clone());

    // Commit the next snapshot first. The refresh must preserve the old one
    // when PostgreSQL cannot answer, then load this committed replacement.
    denylist::replace(
        &db.pool,
        &[denylist::UserEntry {
            wallet: "0xReplacement".into(),
        }],
    )
    .await
    .unwrap();
    let mut blocker = db.pool.begin().await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("LOCK TABLE explore_api.denylist_users IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();

    let jobs = Jobs::new().with_fleet_jobs(false).with_first_pass_now(true);
    let started = tokio::time::Instant::now();
    // Observe the SQL lock wait before the reload pass times out.
    let timeout = Duration::from_secs(3);
    denylist::spawn_reload(&replica.state, &jobs, Duration::from_millis(100), timeout);
    tokio::time::timeout(timeout - Duration::from_millis(500), async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity
                 WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(blocker_pid)
            .fetch_one(&db.pool)
            .await
            .unwrap();
            if blocked {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the real snapshot query must reach PostgreSQL and wait for the table lock");
    assert_eq!(jobs.report()[0].passes, 0);

    tokio::time::timeout_at(started + timeout + Duration::from_secs(3), async {
        while jobs.report()[0].passes == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the reload deadline must finish a pass despite the SQL lock");
    assert!(started.elapsed() >= timeout);
    let failed = &jobs.report()[0];
    assert!(
        failed
            .last_error
            .as_deref()
            .is_some_and(|error| { error.contains("denylist snapshot reload timed out") }),
        "expected the reload deadline, not a PostgreSQL/pool failure: {failed:?}"
    );
    assert!(Arc::ptr_eq(&previous, &replica.state.denylist.list()));
    assert!(replica.state.denylist.contains("0xprevious"));
    assert!(!replica.state.denylist.contains("0xreplacement"));
    let (status, body) = call_json(&app, "GET", "/config/denylist.json", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"users": [{"wallet": "0xPrevious"}]}));

    blocker.rollback().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let report = &jobs.report()[0];
            if report.passes >= 2
                && report
                    .progress
                    .as_deref()
                    .is_some_and(|message| message.contains(" ok in "))
                && replica.state.denylist.contains("0xreplacement")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the same job must retry and recover after PostgreSQL unblocks");
    assert!(!replica.state.denylist.contains("0xprevious"));
    let (_, body) = call_json(&app, "GET", "/config/denylist.json", None).await;
    assert_eq!(body, json!({"users": [{"wallet": "0xReplacement"}]}));
    assert!(jobs.shutdown(Duration::from_secs(2)).await.is_empty());
    db.teardown().await;
}

#[test]
fn production_denylist_reload_has_thirty_second_interval_and_five_second_budget() {
    assert_eq!(denylist::RELOAD_INTERVAL, Duration::from_secs(30));
    assert_eq!(denylist::RELOAD_TIMEOUT, Duration::from_secs(5));
}

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
    s.sources.feature_flags = "https://gateway.example.org/api/explorer.json".into();
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
    assert_eq!(d.all().len(), 5);
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

/// With no Registry or asset-bundle CDN configured (an island, or a bare
/// node) neither reader sends a request: the boot entity reads know no
/// converted entity, the other Registry reads are unavailable and every
/// object is this node's own.
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
    let (status, _) = call_json(
        &app,
        "GET",
        "/asset-bundle-registry/entities/status/bafy",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
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

    let ping = app
        .clone()
        .oneshot(
            axum::http::Request::get("/realm-provider-ea/ping")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ping.status(), StatusCode::OK);
    assert_eq!(
        axum::body::to_bytes(ping.into_body(), 64).await.unwrap(),
        "/ping"
    );

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
async fn registry_gateway_preserves_queries_and_never_caches_native_reads() {
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    let app = router(node.state.clone());
    for operation in ["active", "versions"] {
        let path = format!("/asset-bundle-registry/entities/{operation}?world_name=Foo.dcl.eth");
        for _ in 0..2 {
            let (status, _) =
                call_json(&app, "POST", &path, Some(json!({"pointers":["0,0"]}))).await;
            assert_eq!(status, StatusCode::OK);
        }
        assert_eq!(mock.hits(&format!("entities/{operation}")), 2);
        assert_eq!(
            mock.bodies.lock().last().unwrap()["query"]["world_name"],
            "Foo.dcl.eth"
        );
    }
    mock.entities.lock().clear();
    let (status, docs) = call_json(
        &app,
        "POST",
        "/asset-bundle-registry/entities/active",
        Some(json!({"pointers":["0,0"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        docs,
        json!([]),
        "native changes must be visible immediately"
    );
}

#[test]
fn registry_backend_is_native_only_on_loopback() {
    use registry::Backend;
    for native in [
        "http://127.0.0.1:5147",
        "https://127.0.0.2:5147",
        "http://[::1]:5147",
        "http://localhost:5147/",
    ] {
        assert!(
            matches!(Backend::parse(native).unwrap(), Backend::Native(_)),
            "{native}"
        );
    }
    // Same-network hosts are not the local runtime: they take the source guard.
    for upstream in [
        "https://asset-bundle-registry.decentraland.org",
        "http://10.0.0.5:5147",
        "http://[fd00::1]:5147",
        "http://catalyst-abgen:5147",
    ] {
        assert!(
            matches!(Backend::parse(upstream).unwrap(), Backend::Upstream(_)),
            "{upstream}"
        );
    }
    for bad in [
        "ftp://127.0.0.1/",
        "http://user:pw@127.0.0.1/",
        "http://127.0.0.1/?token=x",
        "http://127.0.0.1/#x",
        "not a url",
    ] {
        assert!(Backend::parse(bad).is_err(), "{bad}");
    }
}

/// A read queues for a backend slot instead of failing at once, and is
/// refused only when none frees up within the wait.
#[tokio::test]
async fn registry_reads_wait_briefly_for_a_backend_slot() {
    use tokio::sync::Semaphore;
    let permits = Semaphore::new(1);
    let held = permits.acquire().await.unwrap();
    assert!(
        registry::slot(&permits, Duration::from_millis(20))
            .await
            .is_none()
    );
    let waiting = registry::slot(&permits, Duration::from_secs(2));
    let release = async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(held);
    };
    let (permit, ()) = tokio::join!(waiting, release);
    assert!(permit.is_some());
}

/// Startup validation accepts the local runtime over http or https and a
/// public Registry, and names the variable when it refuses one.
#[test]
fn registry_source_validates_at_startup_and_names_the_variable() {
    let mut settings = Settings::for_tests(Sources::defaults());
    settings.allow_private_sources = false;
    settings.public_url = "https://gateway.example.org".into();
    for ok in [
        "http://127.0.0.1:5147",
        "https://127.0.0.1:5147",
        "http://localhost:5147",
        "https://asset-bundle-registry.decentraland.org",
    ] {
        let mut s = settings.clone();
        s.sources.registry = ok.into();
        s.validate().unwrap_or_else(|e| panic!("{ok}: {e:#}"));
    }
    for bad in [
        "http://10.0.0.5:5147",
        "http://catalyst-abgen:5147",
        "http://user:pw@127.0.0.1:5147",
        "https://gateway.example.org/asset-bundle-registry",
    ] {
        let mut s = settings.clone();
        s.sources.registry = bad.into();
        let err = format!("{:#}", s.validate().unwrap_err());
        assert!(err.contains("EXPLORER_REGISTRY_SOURCE_URL"), "{bad}: {err}");
    }
}

#[tokio::test]
async fn registry_profiles_are_composed_from_the_shared_profile_service() {
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    let app = router(node.state.clone());
    for path in ["/profiles", "/profiles/metadata"] {
        let (status, _, _) = call(
            &app,
            "POST",
            &format!("/asset-bundle-registry{path}"),
            Some(json!({"ids": []})),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    assert!(mock.hits.lock().is_empty());
    let (status, st) = call_json(&app, "GET", "/asset-bundle-registry/status", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        key_set(&st["data"]),
        ["commitHash", "currentTime", "version"]
            .into_iter()
            .map(String::from)
            .collect()
    );
}

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
    assert!(body["message"] == "Registry service unavailable", "{body}");
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
async fn denylist_refresh_reaches_every_replica_without_mirrors_or_fleet_jobs() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let mut settings = Settings::for_tests(sources_at(addr));
    settings.mirrors_enabled = false;
    settings.ab_cdn_enabled = false;
    let first = node_with_db(settings.clone(), db.pool.clone()).await;
    let second = node_with_db(settings, db.pool.clone()).await;
    let snapshots = [&first.state.denylist, &second.state.denylist];
    assert!(!Arc::ptr_eq(snapshots[0], snapshots[1]));
    // Each replica's social referral readers retain this same Arc.
    let referral_readers = [snapshots[0].clone(), snapshots[1].clone()];
    let first_jobs = Jobs::new().with_fleet_jobs(false);
    let second_jobs = Jobs::new().with_fleet_jobs(false);
    first.state.spawn_jobs(&first_jobs);
    second.state.spawn_jobs(&second_jobs);
    for jobs in [&first_jobs, &second_jobs] {
        assert!(
            jobs.report()
                .iter()
                .any(|j| j.name == "explorer-denylist-reload")
        );
        assert!(jobs.report().iter().all(|j| !j.name.ends_with("-mirror")));
    }

    async fn refresh(jobs: [&Jobs; 2], expected_passes: u64) {
        // Advance only the between-pass sleep, then allow real PostgreSQL IO
        // with normal time so no query budget can auto-expire in paused time.
        tokio::task::yield_now().await;
        tokio::time::pause();
        tokio::time::advance(denylist::RELOAD_INTERVAL + Duration::from_millis(1)).await;
        tokio::time::resume();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if jobs.iter().all(|jobs| {
                    jobs.report().iter().any(|job| {
                        job.name == "explorer-denylist-reload"
                            && job.passes >= expected_passes
                            && job.last_error.is_none()
                    })
                }) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("every replica must refresh without a fleet lease");
    }

    // A mirror's transaction and an admin's overlay write are shared facts;
    // neither writer refreshes either process's snapshot in this test.
    denylist::replace(
        &db.pool,
        &[denylist::UserEntry {
            wallet: "0xMirrored".into(),
        }],
    )
    .await
    .unwrap();
    denylist::set_local(&db.pool, "0xLocal", true)
        .await
        .unwrap();
    assert!(referral_readers.iter().all(|s| !s.contains("0xlocal")));
    refresh([&first_jobs, &second_jobs], 1).await;
    for snapshot in &referral_readers {
        assert!(snapshot.contains("0xmirrored"));
        assert!(snapshot.contains("0xlocal"));
    }

    denylist::set_local(&db.pool, "0xMirrored", false)
        .await
        .unwrap();
    denylist::clear_local(&db.pool, "0xLocal").await.unwrap();
    denylist::replace(
        &db.pool,
        &[
            denylist::UserEntry {
                wallet: "0xMirrored".into(),
            },
            denylist::UserEntry {
                wallet: "0xNew".into(),
            },
        ],
    )
    .await
    .unwrap();
    refresh([&first_jobs, &second_jobs], 2).await;
    for snapshot in &referral_readers {
        assert!(!snapshot.contains("0xmirrored"), "local removal wins");
        assert!(
            !snapshot.contains("0xlocal"),
            "cleared local addition disappears"
        );
        assert!(snapshot.contains("0xnew"), "new mirror rows become visible");
    }
    for state in [&first.state, &second.state] {
        let (_, doc) =
            call_json(&router(state.clone()), "GET", "/config/denylist.json", None).await;
        assert_eq!(doc, json!({"users": [{"wallet": "0xNew"}]}));
    }
    assert_eq!(
        mock.hits("denylist"),
        0,
        "refresh only reads the shared database"
    );
    assert!(first_jobs.shutdown(Duration::from_secs(1)).await.is_empty());
    assert!(
        second_jobs
            .shutdown(Duration::from_secs(1))
            .await
            .is_empty()
    );
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
    feature_flags::reload(&node.state.flags, &db.pool, "explorer")
        .await
        .unwrap();
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

/// Review architecture-1 explore-change-4: the pass also mirrors the
/// `events` document, so `events::foundation` reads a real row.
#[tokio::test]
async fn feature_flags_mirror_stores_the_events_document_for_the_foundation_names() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let node = node_with_db(Settings::for_tests(sources_at(addr)), db.pool.clone()).await;
    let (jobs, ctx) = job_ctx().await;

    feature_flags::run_pass(&node.state, &ctx).await.unwrap();
    assert_eq!(
        mock.hits("events-flags"),
        1,
        "the referer-gated events document"
    );
    let (stored, etag): (Value, Option<String>) =
        sqlx::query_as("SELECT document, etag FROM explore_api.feature_flags WHERE app = 'events'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(stored, *mock.events_flags.lock());
    assert_eq!(etag.as_deref(), Some("\"events-flags-1\""));

    let foundation = crate::events::foundation::Foundation::default();
    foundation.refresh_if_stale(&db.pool).await;
    assert_eq!(
        crate::events::foundation::display_name(
            &foundation.snapshot(),
            Some("0xf0000000000000000000000000000000000000f0"),
            Some("Alice".into())
        ),
        Some(crate::events::foundation::FOUNDATION_NAME.into())
    );
    let (explorer,): (Value,) =
        sqlx::query_as("SELECT document FROM explore_api.feature_flags WHERE app = 'explorer'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(
        explorer["variants"]
            .get("events-decentraland-foundation-addresses")
            .is_none(),
        "the two documents stay apart"
    );

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

#[tokio::test]
async fn dapps_flags_refresh_preserves_the_audience_and_last_good_document() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let reply = Arc::new(Mutex::new((
        StatusCode::OK,
        json!({
            "flags": {"dapps-communities_global_moderators": true},
            "variants": {"dapps-communities_global_moderators": {
                "payload": {"value": "0xABC"}
            }}
        }),
    )));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let (reply_route, observed_route) = (reply.clone(), observed.clone());
    let server = Router::new().route(
        "/dapps.json",
        get(move |headers: HeaderMap| {
            let (reply, observed) = (reply_route.clone(), observed_route.clone());
            async move {
                observed.lock().push(headers);
                let (status, body) = reply.lock().clone();
                (status, Json(body))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(axum::serve(listener, server).into_future());
    let node = node_with_db(Settings::for_tests(sources_at(addr)), db.pool.clone()).await;
    let snapshot = feature_flags::Snapshot::empty();
    feature_flags::reload(&snapshot, &db.pool, "dapps")
        .await
        .unwrap();
    assert!(
        !snapshot.is_sourced(),
        "a fresh database holds no dapps document from the service"
    );
    let url = format!("http://{addr}/dapps.json");
    let audience = "https://social.example.invalid";
    feature_flags::refresh_app(
        &node.state,
        &snapshot,
        "dapps",
        &url,
        audience,
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    assert_eq!(observed.lock()[0][header::REFERER], audience);
    assert!(observed.lock()[0].get(header::IF_NONE_MATCH).is_none());
    assert!(snapshot.is_sourced(), "a fetched document is sourced");
    assert_eq!(
        snapshot.variant_values("dapps", "communities_global_moderators"),
        Some(vec!["0xabc".into()])
    );
    let last_good = snapshot.document();
    for invalid in [
        (StatusCode::SERVICE_UNAVAILABLE, json!({})),
        (
            StatusCode::OK,
            json!({"flags":{"invalid":"true"}, "variants":{}}),
        ),
    ] {
        *reply.lock() = invalid;
        assert!(
            feature_flags::refresh_app(
                &node.state,
                &snapshot,
                "dapps",
                &url,
                audience,
                Duration::from_secs(1)
            )
            .await
            .is_err()
        );
        assert_eq!(snapshot.document(), last_good);
        assert!(
            snapshot.is_sourced(),
            "a failed refresh keeps the sourced document"
        );
        let restarted = feature_flags::Snapshot::empty();
        feature_flags::reload(&restarted, &db.pool, "dapps")
            .await
            .unwrap();
        assert_eq!(restarted.document(), last_good);
        assert!(
            restarted.is_sourced(),
            "a stored document is sourced after a restart"
        );
    }
    task.abort();
    db.teardown().await;
}

/// Shop's side of the map build as `serve` hands it over: a fixed snapshot,
/// or the source it is waiting for.
struct FixedLandMarket(Mutex<Result<map::LandMarketSnapshot, crate::chain_index::source::Source>>);

impl map::LandMarket for FixedLandMarket {
    fn snapshot<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        chain_id: i64,
        _land: &'a str,
        _estate: &'a str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> futures::future::BoxFuture<'a, Result<map::LandMarketSnapshot, map::LandMarketError>> {
        assert_eq!(chain_id, 1, "the test LAND index is mainnet's");
        let answer = match &*self.0.lock() {
            Ok(snapshot) => Ok(snapshot.clone()),
            Err(source) => Err(map::LandMarketError::Unready(
                *source,
                "fixture not published".into(),
            )),
        };
        Box::pin(async move {
            // The build's clock is the snapshot's own, never the process's.
            let on_snapshot_clock: bool = sqlx::query_scalar("SELECT now() = $1")
                .bind(now)
                .fetch_one(&mut *conn)
                .await
                .unwrap();
            assert!(on_snapshot_clock, "the build's now is the snapshot's now()");
            answer
        })
    }
}

/// A Shop side that, once called on the build's snapshot, commits a rewind
/// of the LAND index on another connection (its publication withdrawn, the
/// estate links deleted), then counts the estate links on the snapshot.
struct RewindingLandMarket {
    pool: PgPool,
    links_seen: Mutex<Option<i64>>,
}

impl map::LandMarket for RewindingLandMarket {
    fn snapshot<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        _chain_id: i64,
        _land: &'a str,
        _estate: &'a str,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> futures::future::BoxFuture<'a, Result<map::LandMarketSnapshot, map::LandMarketError>> {
        Box::pin(async move {
            sqlx::query("UPDATE land_authz.index_cursor SET folded_block = NULL, folded_at = NULL")
                .execute(&self.pool)
                .await
                .unwrap();
            sqlx::query("DELETE FROM land_authz.estate_land")
                .execute(&self.pool)
                .await
                .unwrap();
            let links: i64 = sqlx::query_scalar("SELECT count(*) FROM land_authz.estate_land")
                .fetch_one(&mut *conn)
                .await
                .unwrap();
            *self.links_seen.lock() = Some(links);
            Ok(map::LandMarketSnapshot::default())
        })
    }
}

/// A Shop side whose next call, once armed, signals `pinned` on the build's
/// snapshot and holds the build there until `release`.
struct GatedLandMarket {
    answer: Mutex<map::LandMarketSnapshot>,
    gate: Mutex<Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>>,
}

impl map::LandMarket for GatedLandMarket {
    fn snapshot<'a>(
        &'a self,
        _conn: &'a mut sqlx::PgConnection,
        _chain_id: i64,
        _land: &'a str,
        _estate: &'a str,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> futures::future::BoxFuture<'a, Result<map::LandMarketSnapshot, map::LandMarketError>> {
        let answer = self.answer.lock().clone();
        let gate = self.gate.lock().take();
        Box::pin(async move {
            if let Some((pinned, release)) = gate {
                pinned.notify_one();
                release.notified().await;
            }
            Ok(answer)
        })
    }
}

fn land_authz_on(pool: &PgPool) -> Arc<crate::land_authz::LandAuthzState> {
    Arc::new(crate::land_authz::LandAuthzState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        crate::land_authz::Settings::for_tests(),
    ))
}

/// The LAND fixture: estate 7 (two parcels at 10,13 and 11,13, owned by
/// `0xbb..`, named "Seven"), a lone parcel at 10,14 owned by `0xaa..`; all
/// off the roads and plazas.
async fn seed_land(pool: &PgPool, settings: &crate::land_authz::Settings) {
    let owner_a = format!("0x{}", "a".repeat(40));
    let owner_b = format!("0x{}", "b".repeat(40));
    let token = |x: i32, y: i32| map::meta::encode_land_token_id(x, y);
    for (x, y, owner) in [
        (10, 13, settings.estate_registry.clone()),
        (11, 13, settings.estate_registry.clone()),
        (10, 14, owner_a.clone()),
    ] {
        sqlx::query(
            "INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) \
             VALUES ($1, $2::numeric, $3, $4, $5, 90, 0)",
        )
        .bind(&settings.land_registry)
        .bind(token(x, y))
        .bind(x)
        .bind(y)
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) \
         VALUES (90, 0, 1700000000, $1, 'transfer', $2::numeric, $3)",
    )
    .bind(&settings.land_registry)
    .bind(token(10, 14))
    .bind(&owner_a)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO land_authz.token_right (token_address, token_id, owner, updated_block, updated_log) \
         VALUES ($1, 7, $2, 91, 0)",
    )
    .bind(&settings.estate_registry)
    .bind(&owner_b)
    .execute(pool)
    .await
    .unwrap();
    for (x, y) in [(10, 13), (11, 13)] {
        sqlx::query(
            "INSERT INTO land_authz.estate_land (land_token_id, estate_token_id, updated_block, updated_log) \
             VALUES ($1::numeric, 7, 91, 1)",
        )
        .bind(token(x, y))
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO land_authz.estate_data_event (block_number, log_index, token_address, estate_id, name) \
         VALUES (91, 2, $1, 7, 'Seven')",
    )
    .bind(&settings.estate_registry)
    .execute(pool)
    .await
    .unwrap();
}

async fn publish_land(pool: &PgPool, settings: &crate::land_authz::Settings) {
    sqlx::query(
        "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at) \
         VALUES ($1, $2, $3, 100, 100, timezone('UTC', now())) \
         ON CONFLICT (network) DO UPDATE SET folded_block = 100, last_block = 100, folded_at = EXCLUDED.folded_at",
    )
    .bind(&settings.network)
    .bind(&settings.land_registry)
    .bind(&settings.estate_registry)
    .execute(pool)
    .await
    .unwrap();
}

fn estate_order(settings: &crate::land_authz::Settings, mana: u64) -> map::build::LandOrder {
    map::build::LandOrder {
        contract_address: settings.estate_registry.clone(),
        token_id: "7".into(),
        open: true,
        seller: format!("0x{}", "b".repeat(40)),
        price: format!("{mana}000000000000000000"),
        expires_at_ms: 4_102_444_800_000,
        created_at: 1_700_000_100,
        updated_at: 1_700_000_100,
    }
}

async fn stored_grid(pool: &PgPool) -> Option<(i64, i32, chrono::NaiveDateTime)> {
    sqlx::query_as(
        "SELECT last_updated_at, tile_count, fetched_at FROM explore_api.map_grid WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// No grid is held or stored, and every map route answers the unavailable
/// envelope naming `source` (and the variable `configure`, when one is
/// named), uncached and without a grid's validators: never an empty or
/// partial map.
async fn assert_no_map(app: &Router, pool: &PgPool, source: &str, configure: Option<&str>) {
    for path in [
        "/api/v2/tiles",
        "/api/v2/tiles?x1=10&x2=11&y1=13&y2=14",
        "/api/v1/tiles",
        "/api/v2/tiles/info",
        "/api/v2/parcels/10/14",
        "/api/v2/estates/7",
        "/api/ready",
        "/api/v1/map.png?width=64&height=64&center=0,0&size=5",
        "/api/v1/minimap.png",
    ] {
        let (status, headers, body) = call(app, "GET", path, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(headers["retry-after"], "60", "{path}");
        assert_eq!(headers["cache-control"], "no-store", "{path}");
        assert!(
            !headers.contains_key("etag") && !headers.contains_key("last-modified"),
            "{path}: no grid validators"
        );
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"], "Service Unavailable", "{path}: {body}");
        assert_eq!(body["source"], source, "{path}: {body}");
        assert_eq!(
            body.get("configure").and_then(Value::as_str),
            configure,
            "{path}: {body}"
        );
    }
    // A conditional request gets the envelope too, never a 304.
    for path in [
        "/api/v2/tiles",
        "/api/v2/parcels/10/14",
        "/api/v1/minimap.png",
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("if-none-match", "*")
                    .header("if-modified-since", "Thu, 01 Jan 2099 00:00:00 GMT")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(resp.headers()["cache-control"], "no-store", "{path}");
        assert!(
            !resp.headers().contains_key("etag") && !resp.headers().contains_key("last-modified"),
            "{path}"
        );
    }
    assert_eq!(
        stored_grid(pool).await,
        None,
        "nothing stored while {source} waits"
    );
}

#[tokio::test]
async fn map_grid_is_built_from_the_land_index_and_shop_and_waits_for_unpublished_sources() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let node = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    let app = router(node.state.clone());
    let (jobs, ctx) = job_ctx().await;

    // No grid yet and no build has recorded what it waits for: every map
    // route answers the envelope naming the grid itself, never an empty map.
    assert_no_map(&app, &db.pool, "map-grid", None).await;

    // Nothing attached, then an index that has not published: no grid.
    assert!(map::run_pass(&node.state, &ctx).await.is_err());
    let settings = crate::land_authz::Settings::for_tests();
    node.state.attach_land_authz(land_authz_on(&db.pool));
    let market = Arc::new(FixedLandMarket(Mutex::new(Err(
        crate::chain_index::source::Source::NftIndex,
    ))));
    node.state.attach_land_market(market.clone());
    seed_land(&db.pool, &settings).await;
    let err = map::run_pass(&node.state, &ctx).await.unwrap_err();
    assert!(
        err.to_string().contains("land-authz-index not ready"),
        "{err}"
    );
    assert_no_map(
        &app,
        &db.pool,
        "land-authz-index",
        Some("LAND_AUTHZ_INDEX_ENABLED"),
    )
    .await;

    // The LAND index published, Shop's order or trade tables not yet: still
    // no grid (not one without prices), and the envelope names what the pass
    // waits for.
    publish_land(&db.pool, &settings).await;
    *market.0.lock() = Err(crate::chain_index::source::Source::TradesMirror);
    let err = map::run_pass(&node.state, &ctx).await.unwrap_err();
    assert!(err.to_string().contains("trades-mirror not ready"), "{err}");
    assert_no_map(
        &app,
        &db.pool,
        "trades-mirror",
        Some("TRADES_SYNC_UPSTREAM_URL"),
    )
    .await;
    *market.0.lock() = Err(crate::chain_index::source::Source::NftIndex);
    let err = map::run_pass(&node.state, &ctx).await.unwrap_err();
    assert!(err.to_string().contains("nft-index not ready"), "{err}");
    assert_no_map(
        &app,
        &db.pool,
        "nft-index",
        Some("MARKETPLACE_NFT_INDEX_ENABLED"),
    )
    .await;
    // A process that does not run the build names the source the build
    // recorded once it follows the pass marker.
    let follower = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    let follower_app = router(follower.state.clone());
    assert_no_map(&follower_app, &db.pool, "map-grid", None).await;
    map::follow(&follower.state).await.unwrap();
    assert_no_map(
        &follower_app,
        &db.pool,
        "nft-index",
        Some("MARKETPLACE_NFT_INDEX_ENABLED"),
    )
    .await;
    let (_, report) = call_json(&app, "GET", "/mirrors", None).await;
    let row = report["mirrors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mirror"] == "map-grid")
        .unwrap()
        .clone();
    assert_eq!(row["complete"], false);
    assert!(row["lastError"].as_str().unwrap().contains("nft-index"));

    // Every source published: the grid is built, stored and served.
    *market.0.lock() = Ok(map::LandMarketSnapshot {
        orders: vec![estate_order(&settings, 1500)],
        ..Default::default()
    });
    map::run_pass(&node.state, &ctx).await.unwrap();
    let stamp = node.state.map.last_updated_at();
    assert!(stamp > 0);
    let (status, info) = call_json(&app, "GET", "/api/v2/tiles/info", None).await;
    assert_eq!(
        (status, info),
        (StatusCode::OK, json!({"lastUpdatedAt": stamp}))
    );
    let (status, tiles) =
        call_json(&app, "GET", "/api/v2/tiles?x1=10&x2=11&y1=13&y2=14", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        key_set(&tiles["data"]),
        ["10,13", "10,14", "11,13"]
            .into_iter()
            .map(String::from)
            .collect()
    );
    let estate_tile = &tiles["data"]["11,13"];
    assert_eq!(estate_tile["type"], "owned");
    assert_eq!(estate_tile["owner"], format!("0x{}", "b".repeat(40)));
    assert_eq!(estate_tile["estateId"], "7");
    assert_eq!(estate_tile["name"], "Seven");
    assert_eq!(estate_tile["left"], true, "joined to 10,13");
    assert_eq!(estate_tile["price"], 1500.0);
    assert_eq!(estate_tile["expiresAt"], 4_102_444_800_i64);
    let lone = &tiles["data"]["10,14"];
    assert_eq!(lone["owner"], format!("0x{}", "a".repeat(40)));
    assert_eq!(lone["tokenId"], map::meta::encode_land_token_id(10, 14));
    assert_eq!(
        lone["updatedAt"], 1_700_000_000_000_i64,
        "its last rights event"
    );
    assert!(lone.get("price").is_none());
    let (status, plaza) = call_json(&app, "GET", "/api/v2/tiles?x1=0&x2=0&y1=0&y2=0", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        plaza["data"]["0,0"]["type"], "plaza",
        "the roads and plazas are on the grid"
    );
    let (status, legacy) =
        call_json(&app, "GET", "/api/v1/tiles?x1=11&x2=11&y1=13&y2=13", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(legacy["data"]["11,13"]["type"], 10, "a priced tile");
    let (status, headers, png) = call(
        &app,
        "GET",
        "/api/v1/map.png?width=64&height=64&center=10,13&size=5",
        None,
    )
    .await;
    assert_eq!(
        (status, &headers["content-type"]),
        (
            StatusCode::OK,
            &"image/png".parse::<axum::http::HeaderValue>().unwrap()
        )
    );
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let (status, estate) = call_json(&app, "GET", "/api/v2/estates/7", None).await;
    assert_eq!((status, &estate["name"]), (StatusCode::OK, &json!("Seven")));
    let (stored_stamp, count, built_at) = stored_grid(&db.pool).await.unwrap();
    assert_eq!(stored_stamp, stamp);
    assert_eq!(
        count as usize,
        node.state.map.snapshot().unwrap().tiles.len()
    );
    let (_, report) = call_json(&app, "GET", "/mirrors", None).await;
    let row = report["mirrors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mirror"] == "map-grid")
        .unwrap()
        .clone();
    assert_eq!(row["complete"], true);
    assert_eq!(row["cursor"], stamp.to_string());
    assert_eq!(report["mapGridLastUpdatedAt"], stamp);
    map::follow(&follower.state).await.unwrap();
    assert_eq!(
        follower.state.map.last_updated_at(),
        stamp,
        "the follower serves it"
    );

    // Unchanged sources: nothing is written and the stamp stays.
    map::run_pass(&node.state, &ctx).await.unwrap();
    assert_eq!(node.state.map.last_updated_at(), stamp);
    assert_eq!(
        stored_grid(&db.pool).await.unwrap().2,
        built_at,
        "no rewrite"
    );

    // A restart serves the stored grid before its first pass, and that pass
    // finds it unchanged.
    let restarted = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    assert!(restarted.state.map.is_ready());
    assert_eq!(restarted.state.map.last_updated_at(), stamp);
    restarted
        .state
        .attach_land_authz(node.state.land_authz.get().unwrap().clone());
    restarted.state.attach_land_market(market.clone());
    map::run_pass(&restarted.state, &ctx).await.unwrap();
    assert_eq!(restarted.state.map.last_updated_at(), stamp);
    assert_eq!(stored_grid(&db.pool).await.unwrap().2, built_at);

    // A changed listing moves the stamp at least a second forward, and a
    // process that did not build it reloads it.
    *market.0.lock() = Ok(map::LandMarketSnapshot::default());
    map::run_pass(&node.state, &ctx).await.unwrap();
    let moved = node.state.map.last_updated_at();
    assert!(moved >= stamp + 1000, "{moved} after {stamp}");
    let replica = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    assert!(
        !map::reload_if_newer(&replica.state).await.unwrap(),
        "already current"
    );
    assert!(map::reload_if_newer(&restarted.state).await.unwrap());
    assert_eq!(restarted.state.map.last_updated_at(), moved);
    assert!(!map::reload_if_newer(&restarted.state).await.unwrap());
    let (_, tiles) = call_json(&app, "GET", "/api/v2/tiles?x1=11&x2=11&y1=13&y2=13", None).await;
    assert!(tiles["data"]["11,13"].get("price").is_none());
    assert_eq!(stored_grid(&db.pool).await.unwrap().0, moved);

    // A source that stops publishing keeps the held grid served; the pass
    // fails naming it.
    *market.0.lock() = Err(crate::chain_index::source::Source::TradesMirror);
    let err = map::run_pass(&node.state, &ctx).await.unwrap_err();
    assert!(err.to_string().contains("trades-mirror not ready"), "{err}");
    let (status, _) = call_json(&app, "GET", "/api/v2/tiles?x1=11&x2=11&y1=13&y2=13", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(node.state.map.last_updated_at(), moved);

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

/// The stored row is the grid's revision, not what a process holds: a lease
/// successor holding an older grid compares with the row, a slow reload of
/// an older revision never replaces a newer one, and a build overtaken by
/// another publisher's write stores nothing.
#[tokio::test]
async fn map_grid_revisions_follow_the_stored_row_not_the_process() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let (jobs, ctx) = job_ctx().await;
    let settings = crate::land_authz::Settings::for_tests();
    seed_land(&db.pool, &settings).await;
    publish_land(&db.pool, &settings).await;
    let priced = map::LandMarketSnapshot {
        orders: vec![estate_order(&settings, 1500)],
        ..Default::default()
    };
    let node = |market: map::LandMarketSnapshot| {
        let pool = db.pool.clone();
        async move {
            let n = node_with_db(Settings::for_tests(Sources::defaults()), pool.clone()).await;
            n.state.attach_land_authz(land_authz_on(&pool));
            let market = Arc::new(FixedLandMarket(Mutex::new(Ok(market))));
            n.state.attach_land_market(market.clone());
            (n, market)
        }
    };
    let digest = |stored: &map::Grid| hex::encode(stored.digest);
    let stored_etag = || async {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT etag FROM explore_api.map_grid WHERE id = 1",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap()
    };

    // A builds X; C starts holding X; B then builds Y.
    let (a, _) = node(priced.clone()).await;
    map::run_pass(&a.state, &ctx).await.unwrap();
    let x = a.state.map.snapshot().unwrap();
    assert_eq!(
        stored_etag().await,
        Some(digest(&x)),
        "the row's etag is its body's digest"
    );
    let (c, c_market) = node(priced.clone()).await;
    assert_eq!(c.state.map.last_updated_at(), x.last_updated_at);
    let (b, b_market) = node(map::LandMarketSnapshot::default()).await;
    map::run_pass(&b.state, &ctx).await.unwrap();
    let y = b.state.map.snapshot().unwrap();
    assert!(y.last_updated_at > x.last_updated_at);
    assert_ne!(digest(&y), digest(&x));

    // A reload that fetched X before Y was installed never puts X back.
    let slow = map::fetch_stored(&db.pool).await.unwrap().unwrap();
    assert!(map::reload_if_newer(&a.state).await.unwrap());
    assert_eq!(a.state.map.last_updated_at(), y.last_updated_at);
    let stale = map::Grid::from_tiles(x.tiles.clone(), x.last_updated_at, x.fetched_at);
    assert!(
        !a.state.map.install_newer(stale),
        "an older revision is refused"
    );
    assert!(
        !a.state.map.install_newer(slow),
        "the same revision is not reinstalled"
    );
    assert_eq!(a.state.map.last_updated_at(), y.last_updated_at);

    // C, still holding X, builds X again: the row holds Y, so X is stored
    // as a new revision rather than reported unchanged.
    map::run_pass(&c.state, &ctx).await.unwrap();
    let x_again = c.state.map.snapshot().unwrap();
    assert!(x_again.last_updated_at >= y.last_updated_at + 1000);
    assert_eq!(digest(&x_again), digest(&x));
    assert_eq!(
        stored_grid(&db.pool).await.unwrap().0,
        x_again.last_updated_at
    );
    assert_eq!(stored_etag().await, Some(digest(&x)));

    // B, holding Y, builds X too: unchanged against the row, nothing is
    // written, and B serves the stored revision.
    *b_market.0.lock() = Ok(priced.clone());
    let written_at = stored_grid(&db.pool).await.unwrap().2;
    map::run_pass(&b.state, &ctx).await.unwrap();
    assert_eq!(b.state.map.last_updated_at(), x_again.last_updated_at);
    assert_eq!(
        stored_grid(&db.pool).await.unwrap().2,
        written_at,
        "no rewrite"
    );

    // A write guarded on a revision another publisher replaced stores nothing.
    let stored = map::stored_revision(&db.pool).await.unwrap().unwrap();
    let stale_rev = map::Revision {
        stamp: y.last_updated_at,
        etag: Some(digest(&y)),
        built_at: stored.built_at,
    };
    assert!(
        !map::store_guarded(
            &db.pool,
            Some(&stale_rev),
            stored.stamp + 5000,
            "overtaken",
            1,
            b"",
            chrono::Utc::now().naive_utc(),
        )
        .await
        .unwrap()
    );
    assert_eq!(
        map::stored_revision(&db.pool).await.unwrap().unwrap(),
        stored
    );
    assert!(
        !map::store_guarded(
            &db.pool,
            None,
            1,
            "overtaken",
            1,
            b"",
            chrono::Utc::now().naive_utc()
        )
        .await
        .unwrap(),
        "a first write loses to a row stored meanwhile"
    );
    *c_market.0.lock() = Ok(map::LandMarketSnapshot::default());

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

/// A build that read its inputs before another publisher stored a newer
/// grid never replaces it: A pins its snapshot (the stored revision R0, an
/// estate priced 1500) and pauses; B sees the sale and publishes Y; A,
/// released, finds R0 replaced and discards its build. A fresh build whose
/// inputs really return to the old price publishes it again.
#[tokio::test]
async fn an_older_build_never_replaces_a_newer_publication() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let (jobs, ctx) = job_ctx().await;
    let settings = crate::land_authz::Settings::for_tests();
    seed_land(&db.pool, &settings).await;
    publish_land(&db.pool, &settings).await;
    let priced = |mana| map::LandMarketSnapshot {
        orders: vec![estate_order(&settings, mana)],
        ..Default::default()
    };
    let a = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    a.state.attach_land_authz(land_authz_on(&db.pool));
    let a_market = Arc::new(GatedLandMarket {
        answer: Mutex::new(priced(1000)),
        gate: Mutex::new(None),
    });
    a.state.attach_land_market(a_market.clone());
    let b = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    b.state.attach_land_authz(land_authz_on(&db.pool));
    b.state
        .attach_land_market(Arc::new(FixedLandMarket(Mutex::new(
            Ok(Default::default()),
        ))));
    let row = || async {
        sqlx::query_as::<_, (i64, Option<String>)>(
            "SELECT last_updated_at, etag FROM explore_api.map_grid WHERE id = 1",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap()
    };

    // R0: the estate at 1000.
    map::run_pass(&a.state, &ctx).await.unwrap();
    let r0 = row().await;

    // A reads R0 and the estate at 1500 in its snapshot, then pauses.
    *a_market.answer.lock() = priced(1500);
    let (pinned, release) = (
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
    );
    *a_market.gate.lock() = Some((pinned.clone(), release.clone()));
    let paused = {
        let (state, ctx) = (a.state.clone(), ctx.clone());
        tokio::spawn(async move { map::run_pass(&state, &ctx).await })
    };
    pinned.notified().await;

    // B sees the sale (no order) and publishes Y.
    map::run_pass(&b.state, &ctx).await.unwrap();
    let y = row().await;
    assert!(y.0 > r0.0 && y.1 != r0.1);
    let y_grid = b.state.map.snapshot().unwrap();
    assert_eq!(y_grid.tiles["11,13"].price, None);

    // A resumes: its build from the older snapshot is discarded.
    release.notify_one();
    paused.await.unwrap().unwrap();
    assert_eq!(row().await, y, "Y's stamp and digest survive");
    let stored = map::fetch_stored(&db.pool).await.unwrap().unwrap();
    assert_eq!(stored.digest, y_grid.digest, "Y's body survives");
    assert_eq!(stored.tiles["11,13"].price, None);
    let served = a.state.map.snapshot().unwrap();
    assert_eq!(
        (served.last_updated_at, served.digest),
        (y.0, y_grid.digest),
        "A serves Y"
    );
    let marker: (i64, Option<String>) = sqlx::query_as(
        "SELECT items, cursor FROM explore_api.mirror_passes WHERE mirror = 'map-grid'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        marker,
        (served.tiles.len() as i64, Some(y.0.to_string())),
        "the pass reports the revision it serves"
    );

    // A fresh build whose inputs really are 1500 publishes them past Y.
    map::run_pass(&a.state, &ctx).await.unwrap();
    let x = row().await;
    assert!(x.0 >= y.0 + 1000, "{} after {}", x.0, y.0);
    assert_ne!(x.1, y.1);
    assert_eq!(
        a.state.map.snapshot().unwrap().tiles["11,13"].price,
        Some(1500.0)
    );

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

/// A builder whose lease goes while it reads stores nothing, and a changed
/// grid waits until the stored revision is the publication floor old.
#[tokio::test]
async fn a_cancelled_or_too_early_build_stores_nothing() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let (jobs, ctx) = job_ctx().await;
    let settings = crate::land_authz::Settings::for_tests();
    seed_land(&db.pool, &settings).await;
    publish_land(&db.pool, &settings).await;
    let priced = |mana| map::LandMarketSnapshot {
        orders: vec![estate_order(&settings, mana)],
        ..Default::default()
    };
    let mut floored = Settings::for_tests(Sources::defaults());
    floored.map_min_publish_interval = Duration::from_secs(3600);
    let a = node_with_db(floored, db.pool.clone()).await;
    a.state.attach_land_authz(land_authz_on(&db.pool));
    let market = Arc::new(GatedLandMarket {
        answer: Mutex::new(priced(1000)),
        gate: Mutex::new(None),
    });
    a.state.attach_land_market(market.clone());
    let row = || async { stored_grid(&db.pool).await.unwrap() };

    // The first grid has no revision to wait for.
    map::run_pass(&a.state, &ctx).await.unwrap();
    let first = row().await;

    // A changed grid inside the floor is held back; the stored one stays.
    *market.answer.lock() = priced(1500);
    map::run_pass(&a.state, &ctx).await.unwrap();
    assert_eq!(row().await, first, "held back by the floor");
    assert_eq!(a.state.map.last_updated_at(), first.0);
    assert_eq!(
        a.state.map.snapshot().unwrap().tiles["11,13"].price,
        Some(1000.0)
    );

    // Without the floor, a builder that loses its lease mid-read stores
    // nothing; the next one publishes.
    let b = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    b.state.attach_land_authz(land_authz_on(&db.pool));
    b.state.attach_land_market(market.clone());
    let (pinned, release) = (
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
    );
    *market.gate.lock() = Some((pinned.clone(), release.clone()));
    let (lease_jobs, lease) = job_ctx().await;
    let paused = {
        let (state, lease) = (b.state.clone(), lease.clone());
        tokio::spawn(async move { map::run_pass(&state, &lease).await })
    };
    pinned.notified().await;
    lease.cancel.cancel();
    release.notify_one();
    let err = paused.await.unwrap().unwrap_err();
    assert!(err.to_string().contains("cancelled"), "{err}");
    assert_eq!(row().await, first, "a cancelled build stores nothing");
    map::run_pass(&b.state, &ctx).await.unwrap();
    assert!(row().await.0 > first.0);
    assert_eq!(
        b.state.map.snapshot().unwrap().tiles["11,13"].price,
        Some(1500.0)
    );

    lease_jobs.shutdown(Duration::from_secs(1)).await;
    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

/// A leased build stores on its lease's own session. With the stored row
/// locked by another session, the guarded write waits on that session; the
/// lease is lost there (its backend ends) with no other revision stored in
/// between, and the write goes with it: the row keeps the earlier revision.
#[tokio::test]
async fn a_lease_lost_while_the_write_waits_leaves_the_row_unchanged() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let (jobs, ctx) = job_ctx().await;
    let settings = crate::land_authz::Settings::for_tests();
    seed_land(&db.pool, &settings).await;
    publish_land(&db.pool, &settings).await;
    let priced = |mana| map::LandMarketSnapshot {
        orders: vec![estate_order(&settings, mana)],
        ..Default::default()
    };
    let node = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    node.state.attach_land_authz(land_authz_on(&db.pool));
    let market = Arc::new(FixedLandMarket(Mutex::new(Ok(priced(1000)))));
    node.state.attach_land_market(market.clone());
    map::run_pass(&node.state, &ctx).await.unwrap();
    let revision = || async {
        sqlx::query_as::<_, (i64, Option<String>)>(
            "SELECT last_updated_at, etag FROM explore_api.map_grid WHERE id = 1",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap()
    };
    let r0 = revision().await;

    // Another session holds the row, so the next guarded write waits.
    *market.0.lock() = Ok(priced(1500));
    let mut locker = db.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM explore_api.map_grid WHERE id = 1 FOR UPDATE")
        .execute(&mut *locker)
        .await
        .unwrap();

    // One leased build; after a lost lease it does not run again.
    let leased = Jobs::with_leases(
        db.pool.clone(),
        crate::jobs::LeaseSettings {
            namespace: format!("map-grid-lease-{}", std::process::id()),
            poll: Duration::from_secs(60),
            keepalive: Duration::from_secs(30),
        },
    );
    let (done, outcome) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let done = Arc::new(Mutex::new(Some(done)));
    let state = node.state.clone();
    leased.spawn_leased("map-grid-lease-test", move |lease_ctx| {
        let (state, done) = (state.clone(), done.lock().take());
        async move {
            if let Some(done) = done {
                let result = map::run_pass(&state, &lease_ctx).await;
                let _ = done.send(result.map_err(|e| format!("{e:#}")));
            }
            Ok(())
        }
    });

    // The write waits on the row lock, on the session holding the lease.
    let mut waiting_pid = None;
    for _ in 0..600 {
        waiting_pid = sqlx::query_scalar::<_, i32>(
            "SELECT pid FROM pg_stat_activity \
             WHERE datname = current_database() AND wait_event_type = 'Lock' \
               AND query LIKE 'UPDATE explore_api.map_grid%'",
        )
        .fetch_optional(&db.pool)
        .await
        .unwrap();
        if waiting_pid.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let waiting_pid = waiting_pid.expect("the guarded write waits on the row lock");
    let lease_pid: i32 = sqlx::query_scalar(
        "SELECT pid FROM pg_locks \
         WHERE locktype = 'advisory' AND granted \
           AND database = (SELECT oid FROM pg_database WHERE datname = current_database())",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        waiting_pid, lease_pid,
        "the write runs on the lease's session"
    );

    // The lease is lost while the statement waits; no other revision is
    // stored in between. Then the row is released.
    let ended: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
        .bind(lease_pid)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(ended);
    locker.rollback().await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(30), outcome)
        .await
        .expect("the leased build ends")
        .unwrap();
    assert!(
        result.is_err(),
        "the build fails with its lease: {result:?}"
    );
    assert_eq!(
        revision().await,
        r0,
        "the old publication leaves the row unchanged"
    );

    leased.shutdown(Duration::from_secs(5)).await;
    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

/// The build reads Shop's side on the snapshot its LAND rows came from: a
/// rewind committed in between is invisible to it, so the grid it stores is
/// the coherent earlier state, and the next pass finds the index withdrawn,
/// fails and keeps that grid.
#[tokio::test]
async fn a_land_rewind_committed_during_the_build_does_not_mix_into_the_grid() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let (jobs, ctx) = job_ctx().await;
    let settings = crate::land_authz::Settings::for_tests();
    seed_land(&db.pool, &settings).await;
    publish_land(&db.pool, &settings).await;
    let node = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
    node.state.attach_land_authz(land_authz_on(&db.pool));
    let market = Arc::new(RewindingLandMarket {
        pool: db.pool.clone(),
        links_seen: Mutex::new(None),
    });
    node.state.attach_land_market(market.clone());

    map::run_pass(&node.state, &ctx).await.unwrap();
    assert_eq!(
        *market.links_seen.lock(),
        Some(2),
        "Shop's side reads the build's snapshot"
    );
    let grid = node.state.map.snapshot().unwrap();
    assert_eq!(grid.tiles["11,13"].estate_id.as_deref(), Some("7"));
    assert!(
        grid.tiles["11,13"].left,
        "the estate as it was when the snapshot began"
    );
    let stamp = grid.last_updated_at;

    let err = map::run_pass(&node.state, &ctx).await.unwrap_err();
    assert!(
        err.to_string().contains("land-authz-index not ready"),
        "{err}"
    );
    assert_eq!(
        node.state.map.last_updated_at(),
        stamp,
        "the held grid stays"
    );
    assert_eq!(stored_grid(&db.pool).await.unwrap().0, stamp);

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

/// One bounded run of every mirror pass and reader against the real upstreams
/// (`cargo test --lib live_upstream -- --ignored --nocapture`): three small
/// documents. Registry uses the local runtime. Prints the `/mirrors`
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
    let (_, report) = call_json(&app, "GET", "/mirrors", None).await;
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    println!(
        "pois={} denylist={} flags={} variants={}",
        pois["data"].as_array().unwrap().len(),
        deny["users"].as_array().unwrap().len(),
        flags["flags"].as_object().unwrap().len(),
        flags["variants"].as_object().unwrap().len(),
    );
    for m in report["mirrors"].as_array().unwrap() {
        assert_eq!(m["complete"], true, "{m}");
    }

    let restarted = node_with_db(Settings::for_tests(Sources::defaults()), db.pool.clone()).await;
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
            "auth-api-socket-push",
            "explorer-denylist-mirror",
            "explorer-denylist-reload",
            "explorer-flags-mirror",
            "explorer-lists-mirror",
            "explorer-map-grid",
            "explorer-map-grid-reload",
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

#[tokio::test]
async fn registry_native_routes_preserve_wire_contract_and_replace_untrusted_original_path() {
    use axum::http::{Method, Uri};
    async fn echo(
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        body: axum::body::Bytes,
    ) -> Response {
        (
            StatusCode::ACCEPTED,
            [("cache-control", "no-store"), ("retry-after", "3")],
            Json(json!({
                "method":method.as_str(), "uri":uri.to_string(),
                "body":String::from_utf8(body.to_vec()).unwrap(),
                "authorization":headers.get("authorization").is_some(),
                "identity":headers.get("x-identity-auth-chain-0").unwrap().to_str().unwrap(),
                "timestamp":headers.get("x-identity-timestamp").unwrap().to_str().unwrap(),
                "metadata":headers.get("x-identity-metadata").unwrap().to_str().unwrap(),
                "original":headers.get("x-original-path").unwrap().to_str().unwrap(),
                "contentType":headers.get("content-type").unwrap().to_str().unwrap(),
            })),
        )
            .into_response()
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, Router::new().fallback(echo)).into_future());
    let node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    let app = router(node.state);
    for (method, path) in [
        ("POST", "/entities/active"),
        ("POST", "/entities/versions"),
        ("GET", "/entities/status"),
        ("GET", "/entities/status/bafytest"),
        ("GET", "/worlds/Test.dcl.eth/manifest"),
        ("GET", "/denylist"),
        ("GET", "/queues/status"),
    ] {
        let path_and_query = format!("{path}?world_name=A%2Bb.dcl.eth&x=1&x=2");
        let public = format!("/asset-bundle-registry{path_and_query}");
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(&public)
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer operator-test")
                    .header("x-identity-auth-chain-0", "signed-chain")
                    .header("x-identity-timestamp", "1700000000000")
                    .header("x-identity-metadata", "{}")
                    .header("x-original-path", "/forged-path")
                    .body(Body::from(r#"{ "pointers": ["0,0"], "unchanged": true }"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED, "{method} {path}");
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["retry-after"], "3");
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["method"], method);
        assert_eq!(body["uri"], path_and_query);
        assert_eq!(body["original"], format!("/asset-bundle-registry{path}"));
        assert_eq!(
            body["body"],
            r#"{ "pointers": ["0,0"], "unchanged": true }"#
        );
        assert_eq!(
            body["authorization"], false,
            "the admin bearer never leaves the host"
        );
        assert_eq!(body["identity"], "signed-chain");
        assert_eq!(body["timestamp"], "1700000000000");
        assert_eq!(body["metadata"], "{}");
        assert_eq!(body["contentType"], "application/json");
    }
    let (status, _, _) = call(&app, "POST", "/asset-bundle-registry/not-a-route", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Registry writes stay behind the ops console.
    for (method, path) in [
        ("POST", "/denylist/bafytest"),
        ("DELETE", "/denylist/bafytest"),
        ("POST", "/queues/retry"),
        ("POST", "/queues/pause"),
        ("POST", "/queues/resume"),
        ("POST", "/registry"),
        ("DELETE", "/flush-cache"),
    ] {
        let (status, _, _) =
            call(&app, method, &format!("/asset-bundle-registry{path}"), None).await;
        assert!(
            matches!(
                status,
                StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
            ),
            "{method} {path}: {status}"
        );
    }
    // Percent-encoded dot segments must never escape an enumerated route
    // when the outbound URL parser normalizes them.
    for path in [
        "/asset-bundle-registry/entities/status/%2e%2e",
        "/asset-bundle-registry/worlds/%2e%2e/manifest",
    ] {
        let (status, _, _) = call(&app, "GET", path, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
    }
    let (status, _, _) = call(
        &app,
        "DELETE",
        "/asset-bundle-registry/entities/active",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    server.abort();
}

/// The Registry gateway is opt-in: `ABGEN_ADMIN_URL` (the ops console's
/// runtime) does not select it, since an abgen without native Registry
/// state cannot answer these routes.
#[test]
fn registry_source_is_only_the_explicit_variable() {
    let config = crate::config::Config::parse_for_tests(&[
        "catalyst",
        "--database-url",
        "postgres:///catalyst",
        "--abgen-admin-url",
        "http://127.0.0.1:5147",
    ])
    .unwrap();
    assert_eq!(Sources::from_config(&config).registry, "");
    let config = crate::config::Config::parse_for_tests(&[
        "catalyst",
        "--database-url",
        "postgres:///catalyst",
        "--abgen-admin-url",
        "http://127.0.0.1:5147",
        "--explorer-registry-source-url",
        "http://localhost:5150/",
    ])
    .unwrap();
    assert_eq!(
        Sources::from_config(&config).registry,
        "http://localhost:5150"
    );
}

/// A public Registry receives the request only: no signed-fetch headers, no
/// asserted original path, no bearer.
#[tokio::test]
async fn registry_upstream_gets_no_identity_or_path_assertion() {
    async fn echo(headers: HeaderMap) -> Json<Value> {
        let names: Vec<String> = headers.keys().map(|k| k.to_string()).collect();
        Json(json!(names))
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, Router::new().fallback(echo)).into_future());
    // An IPv4-mapped address is not loopback to the classifier, so this is
    // the upstream path (tests allow private sources).
    let sources = Sources {
        registry: format!("http://[::ffff:127.0.0.1]:{}", addr.port()),
        ..sources_at(addr)
    };
    let node = node_without_db(Settings::for_tests(sources)).await;
    let app = router(node.state);
    let response = app
        .oneshot(
            Request::get("/asset-bundle-registry/entities/status/bafy")
                .header("authorization", "Bearer operator-test")
                .header("x-identity-auth-chain-0", "signed-chain")
                .header("x-original-path", "/forged")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let names: Vec<String> = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap(),
    )
    .unwrap();
    for header in [
        "authorization",
        "x-identity-auth-chain-0",
        "x-original-path",
    ] {
        assert!(!names.iter().any(|n| n == header), "{header}: {names:?}");
    }
    server.abort();
}

#[tokio::test]
async fn registry_native_failures_are_preserved_without_following_redirects() {
    async fn response(Query(q): Query<HashMap<String, String>>) -> Response {
        let status = StatusCode::from_u16(q["status"].parse().unwrap()).unwrap();
        (
            status,
            [
                ("content-type", "application/problem+json"),
                ("location", "http://127.0.0.1:1/never-follow"),
            ],
            r#"{"error":"native failure"}"#,
        )
            .into_response()
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server =
        tokio::spawn(axum::serve(listener, Router::new().fallback(response)).into_future());
    let mut node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    // The production pool normally follows redirects; the Registry adapter
    // must explicitly suppress them for this request only.
    Arc::get_mut(&mut node.state).unwrap().http =
        crate::http::Outbound::new(crate::http::HttpClientCfg::serve(4)).unwrap();
    let recorder = dcl_metrics::Registry::new(&[registry::FAMILIES])
        .unwrap()
        .builder(dcl_metrics::PrometheusBuilder::new())
        .unwrap()
        .build_recorder();
    let _recording = dcl_metrics::set_default_local_recorder(&recorder);
    use dcl_metrics::http::{component, instrument};
    let app = instrument(component(router(node.state), "explore"), "serve");
    for status in [307, 400, 401, 403, 404, 429, 500, 503] {
        let (actual, headers, body) = call(
            &app,
            "POST",
            &format!("/asset-bundle-registry/entities/active?status={status}"),
            Some(json!({"pointers":["0,0"]})),
        )
        .await;
        assert_eq!(actual.as_u16(), status);
        assert_eq!(headers["content-type"], "application/problem+json");
        assert_eq!(body, br#"{"error":"native failure"}"#);
    }

    let scrape = recorder.handle().render();
    for (outcome, count) in [
        ("other_response", 1),
        ("client_error", 5),
        ("server_error", 2),
    ] {
        assert!(
            scrape.lines().any(|line| {
                line.starts_with("dcl_registry_backend_requests_total{")
                    && line.contains("backend=\"native\"")
                    && line.contains(&format!("outcome=\"{outcome}\""))
                    && line.ends_with(&format!(" {count}"))
            }),
            "{scrape}"
        );
    }
    assert!(
        scrape.lines().any(|line| {
            line.starts_with("dcl_http_requests_total{")
                && line.contains("component=\"explore\"")
                && line.contains("route=\"/asset-bundle-registry/entities/active\"")
                && line.contains("status_code=\"500\"")
                && line.ends_with(" 1")
        }),
        "{scrape}"
    );
    server.abort();
}

fn abr_capture(name: &str) -> Value {
    let path = format!(
        "{}/services/catalyst/tests/fixtures/conformance/content-lambdas/captures/abr/{name}",
        env!("DCL_ONE_ROOT")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The boot reads apply the Registry's pointer rule before any backend call,
/// with and without a source: `ids` is not read, an absent or empty
/// `pointers` is the captured 400 (keys in the Registry's order), a body
/// that is not JSON or is `null` the http-commons 500, and the pointers are
/// not capped.
#[tokio::test]
async fn registry_boot_reads_take_the_registry_pointer_rule() {
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let empty = Sources {
        registry: String::new(),
        ..sources_at(addr)
    };
    let captured = [
        ("/entities/active", abr_capture("AB-4-empty-pointers.json")),
        (
            "/entities/versions",
            abr_capture("AB-11-missing-pointers.json"),
        ),
    ];
    for sources in [sources_at(addr), empty] {
        let node = node_without_db(Settings::for_tests(sources)).await;
        let app = router(node.state.clone());
        for (path, capture) in &captured {
            let path = format!("/asset-bundle-registry{path}");
            for body in [
                r#"{}"#,
                r#"{"pointers":[]}"#,
                r#"{"pointers":null}"#,
                r#"{"pointers":""}"#,
                r#"{"pointers":{"length":0}}"#,
                r#"{"ids":["bafkreiexample"]}"#,
            ] {
                let response = app
                    .clone()
                    .oneshot(
                        Request::post(&path)
                            .header("content-type", "application/json")
                            .body(Body::from(body))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path} {body}");
                assert_eq!(response.headers()["content-type"], "application/json");
                let bytes = axum::body::to_bytes(response.into_body(), 1024)
                    .await
                    .unwrap();
                let parsed: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(parsed, capture["response"]["body"], "{path} {body}");
                // The Registry's key order and the captured length.
                assert_eq!(
                    bytes.as_ref(),
                    br#"{"ok":false,"message":"No pointers provided"}"#
                );
                assert_eq!(
                    bytes.len().to_string(),
                    capture["response"]["headers"]["content-length"]
                        .as_str()
                        .unwrap()
                );
            }
            // `null.pointers` throws in the Registry's handler.
            for body in ["", "{", "null"] {
                let response = app
                    .clone()
                    .oneshot(Request::post(&path).body(Body::from(body)).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
                let bytes = axum::body::to_bytes(response.into_body(), 1024)
                    .await
                    .unwrap();
                assert_eq!(bytes.as_ref(), br#"{"error":"Internal Server Error"}"#);
            }
            let many: Vec<String> = (0..1001).map(|i| format!("{i},0")).collect();
            let response = app
                .clone()
                .oneshot(
                    Request::post(&path)
                        .header("content-type", "application/json")
                        .body(Body::from(json!({ "pointers": many }).to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
        }
    }
    // Only the two admitted reads (one per path, with a source) reached it.
    assert_eq!(mock.hits("entities/active"), 1);
    assert_eq!(mock.hits("entities/versions"), 1);
}

/// `POST /entities/active` drops a backend entity whose id is on this node's
/// content denylist, leaving the other entities' bytes (content and
/// asset-bundle fields) as received.
#[tokio::test]
async fn registry_active_entities_leave_out_the_content_denylist() {
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let entity = |id: &str, pointer: &str| {
        json!({"id": id, "type": "scene", "timestamp": "1791215990965",
            "deployer": "0x0000000000000000000000000000000000000b02",
            "pointers": [pointer], "content": [], "metadata": {},
            "status": "complete",
            "bundles": {"assets": {"mac": "complete", "windows": "complete", "webgl": "pending"}}})
    };
    let kept = entity("bafkreikept", "1,1");
    let listed = entity("bafkreilisted", "2,2");
    *mock.entities.lock() = vec![kept.clone(), listed.clone()];
    let node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    let app = router(node.state.clone());
    let request = Some(json!({"pointers": ["1,1", "2,2"]}));
    let path = "/asset-bundle-registry/entities/active";
    let (status, all) = call_json(&app, "POST", path, request.clone()).await;
    assert_eq!((status, all), (StatusCode::OK, json!([kept, listed])));
    node.state.content.denylist.add("bafkreilisted");
    let (status, _, bytes) = call(&app, "POST", path, request.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, serde_json::to_vec(&json!([kept])).unwrap());
    node.state.content.denylist.add("bafkreikept");
    let (status, rest) = call_json(&app, "POST", path, request).await;
    assert_eq!((status, rest), (StatusCode::OK, json!([])));
}

/// A backend that answers every read with cache validators and a `304` to
/// any conditional request, as an HTTP cache-aware Registry would; its
/// `/entities/active` answer is a fixed two-entity list.
async fn serve_validating_backend() -> SocketAddr {
    async fn answer(uri: axum::http::Uri, headers: HeaderMap) -> Response {
        let validators = [
            ("etag", "\"backend\""),
            ("last-modified", "Tue, 10 Nov 2026 00:00:00 GMT"),
            ("cache-control", "public, max-age=60"),
        ];
        if headers.contains_key("if-none-match") || headers.contains_key("if-modified-since") {
            return (StatusCode::NOT_MODIFIED, validators).into_response();
        }
        let body = if uri.path() == "/entities/active" {
            json!([{"id": "bafkreikept"}, {"id": "bafkreilisted"}])
        } else {
            json!({})
        };
        (validators, Json(body)).into_response()
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, Router::new().fallback(answer)).into_future());
    addr
}

/// The backend's `ETag` and `Last-Modified` describe its unfiltered answer:
/// `/entities/active` never sends it the client's conditional headers (its
/// `304` would skip the denylist), keeps the validators while the denylist
/// drops nothing and removes them when it does. `Cache-Control` stays, and
/// the other reads still forward the conditional headers.
#[tokio::test]
async fn registry_active_entities_validators_describe_the_filtered_answer() {
    let addr = serve_validating_backend().await;
    let node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    let app = router(node.state.clone());
    let conditional = |request: axum::http::request::Builder| {
        request
            .header("if-none-match", "\"backend\"")
            .header("if-modified-since", "Tue, 10 Nov 2026 00:00:00 GMT")
    };
    let active = |app: Router| async move {
        let response = app
            .oneshot(
                conditional(Request::post("/asset-bundle-registry/entities/active"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"pointers":["1,1","2,2"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        (status, headers, body)
    };

    let (status, h, body) = active(app.clone()).await;
    assert_eq!(status, StatusCode::OK, "no conditional header reached it");
    assert_eq!(
        body,
        json!([{"id": "bafkreikept"}, {"id": "bafkreilisted"}])
    );
    assert_eq!(h["etag"], "\"backend\"");
    assert_eq!(h["last-modified"], "Tue, 10 Nov 2026 00:00:00 GMT");
    assert_eq!(h["cache-control"], "public, max-age=60");

    node.state.content.denylist.add("bafkreilisted");
    let (status, h, body) = active(app.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([{"id": "bafkreikept"}]));
    assert!(h.get("etag").is_none(), "{h:?}");
    assert!(h.get("last-modified").is_none(), "{h:?}");
    assert_eq!(h["cache-control"], "public, max-age=60");

    let response = app
        .oneshot(
            conditional(Request::get("/asset-bundle-registry/entities/status/bafy"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response.headers()["etag"], "\"backend\"");
}

/// The boot reads cap their body at 1 MiB, with or without a source and
/// before any backend call: unity-explorer's largest read (every parcel of a
/// world at the coordinate bound) fits, a body just over the cap is a 413.
#[tokio::test]
async fn registry_boot_reads_cap_the_request_body() {
    let mock = mock();
    let addr = serve_mock(mock.clone()).await;
    let empty = Sources {
        registry: String::new(),
        ..sources_at(addr)
    };
    let grid: Vec<String> = (-150..=150)
        .flat_map(|x| (-150..=150).map(move |y| format!("{x},{y}")))
        .collect();
    let world = json!({ "pointers": grid }).to_string();
    assert_eq!(world.len(), 865_088);
    let over = format!(
        r#"{{"pointers":["0,0"],"pad":"{}"}}"#,
        "x".repeat(1024 * 1024)
    );
    for sources in [sources_at(addr), empty] {
        let node = node_without_db(Settings::for_tests(sources)).await;
        let app = router(node.state.clone());
        for path in ["/entities/active", "/entities/versions"] {
            let path = format!("/asset-bundle-registry{path}");
            for (body, expected) in [
                (world.clone(), StatusCode::OK),
                (over.clone(), StatusCode::PAYLOAD_TOO_LARGE),
            ] {
                let response = app
                    .clone()
                    .oneshot(
                        Request::post(&path)
                            .header("content-type", "application/json")
                            .body(Body::from(body))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), expected, "{path}");
            }
        }
    }
    // Only the in-limit reads with a source reached the backend.
    assert_eq!(mock.hits("entities/active"), 1);
    assert_eq!(mock.hits("entities/versions"), 1);
}

/// `GET /denylist` at the handler: a source's non-200 answer passes through,
/// a 200 that is not an array is a 502, `HEAD` asks the source with `GET`,
/// and the local half's failed read (no database here) is a 500, with or
/// without a source.
#[tokio::test]
async fn registry_denylist_handler_answers_source_failures_and_local_read_errors() {
    use axum::http::{Method, Uri};
    async fn source(method: Method, uri: Uri) -> Response {
        match (method, uri.query()) {
            (_, Some("case=busy")) => (
                StatusCode::SERVICE_UNAVAILABLE,
                [("retry-after", "3")],
                Json(json!({"ok":false,"message":"busy"})),
            )
                .into_response(),
            (_, Some("case=object")) => Json(json!({"not":"an array"})).into_response(),
            (_, Some("case=malformed")) => "{".into_response(),
            // Only a GET gets this distinct status, so HEAD must arrive as GET.
            (Method::GET, Some("case=method")) => StatusCode::CONFLICT.into_response(),
            (_, Some("case=method")) => StatusCode::IM_A_TEAPOT.into_response(),
            _ => Json(json!([])).into_response(),
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, Router::new().fallback(source)).into_future());
    let node = node_without_db(Settings::for_tests(sources_at(addr))).await;
    let app = router(node.state.clone());
    let path = "/asset-bundle-registry/denylist";

    let (status, headers, body) = call(&app, "GET", &format!("{path}?case=busy"), None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(headers["retry-after"], "3");
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"ok":false,"message":"busy"})
    );
    let (status, _, _) = call(&app, "GET", &format!("{path}?case=object"), None).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let (status, _, _) = call(&app, "GET", &format!("{path}?case=malformed"), None).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let (status, _, body) = call(&app, "HEAD", &format!("{path}?case=method"), None).await;
    assert!(body.is_empty());
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "HEAD reaches the source as GET"
    );
    // The source's array is good; the local read has no database.
    let (status, _, body) = call(&app, "GET", path, None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"error":"Internal Server Error"})
    );

    let empty = Sources {
        registry: String::new(),
        ..sources_at(addr)
    };
    let node = node_without_db(Settings::for_tests(empty)).await;
    let (status, _, _) = call(&router(node.state.clone()), "GET", path, None).await;
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "without a source the local rows are the answer, so their read failing is a 500"
    );
}

#[tokio::test]
async fn registry_denylist_reads_local_rows_without_a_source_and_reuses_them() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let mut settings = Settings::for_tests(Sources::defaults());
    settings.sources.registry.clear();
    let node = node_with_db(settings, db.pool.clone()).await;
    for id in ["QmCase", "0,0", "profile-pointer"] {
        crate::content::denylist::add(&db.pool, id, "0xoperator")
            .await
            .unwrap();
    }
    let app = router(node.state);
    let path = "/asset-bundle-registry/denylist";
    let (status, _, first) = call(&app, "GET", path, None).await;
    assert_eq!(status, StatusCode::OK);
    let rows: Value = serde_json::from_slice(&first).unwrap();
    let ids: BTreeSet<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["entity_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, BTreeSet::from(["QmCase", "0,0", "profile-pointer"]));
    for row in rows.as_array().unwrap() {
        assert_eq!(row["created_by"], "0xoperator");
        assert!(row["reason"].is_null());
        assert_eq!(row["created_at"], row["updated_at"]);
    }
    // Closing the pool makes another SELECT fail, so every concurrent success
    // must come from this router's previously loaded local rows.
    db.pool.close().await;
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let app = app.clone();
        requests.spawn(async move { call(&app, "GET", path, None).await });
    }
    while let Some(response) = requests.join_next().await {
        let (status, _, body) = response.unwrap();
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, first);
    }
    let (status, _, body) = call(&app, "HEAD", path, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_empty());
    db.teardown().await;
}
