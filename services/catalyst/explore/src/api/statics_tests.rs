//! The explorer statics against a real postgres and one stub upstream that
//! stands in for the render rig, the places S3 bucket, the event poster
//! bucket, a JSON-RPC node, a metadata host and opensea.decentraland.org:
//! complete passes, conditional re-checks, a failed pass resumed from its
//! durable stamps, pruning and its fuse, the refusal rules, the routes under
//! their gateway prefixes and at the vhost paths, and the OpenSea cache,
//! fallback and staleness rules. Needs CATALYST_TEST_DATABASE_URL; each test
//! gets its own scratch database.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State as AxState};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use parking_lot::Mutex;
use serde_json::{Value, json};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use tiny_skia::Pixmap;
use tower::ServiceExt;

use super::*;
use crate::content::ContentState;
use crate::jobs::Jobs;
use crate::storage::Storage;

#[derive(Default)]
struct Stub {
    hits: Mutex<HashMap<String, usize>>,
    manifest: Mutex<Value>,
    /// "cx/cy.png" / "cx/cy.t.png" -> (bytes, last-modified)
    regions: Mutex<HashMap<String, (Vec<u8>, String)>>,
    failing: Mutex<HashSet<String>>,
    world: Mutex<Option<(Vec<u8>, String)>>,
    /// "poster/<file>" -> (bytes, content-type)
    posters: Mutex<HashMap<String, (Vec<u8>, String)>>,
    /// tokenURI answer per token id hex (eth_call data suffix)
    token_uris: Mutex<HashMap<String, String>>,
    rpc_down: Mutex<bool>,
    metadata: Mutex<HashMap<String, Value>>,
    /// fallback answer per token id
    fallback: Mutex<HashMap<String, (u16, Value)>>,
}

impl Stub {
    fn hit(&self, what: impl Into<String>) {
        *self.hits.lock().entry(what.into()).or_default() += 1;
    }

    fn hits(&self, what: &str) -> usize {
        self.hits.lock().get(what).copied().unwrap_or(0)
    }

    fn hits_prefixed(&self, prefix: &str) -> usize {
        self.hits
            .lock()
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(_, v)| *v)
            .sum()
    }
}

fn abi_string(s: &str) -> String {
    let data = hex::encode(s.as_bytes());
    let padded = format!(
        "{data:0<width$}",
        width = data.len().div_ceil(64).max(1) * 64
    );
    format!("0x{:064x}{:064x}{padded}", 32, s.len())
}

async fn stub_handler(AxState(stub): AxState<Arc<Stub>>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    let headers = req.headers().clone();
    let method = req.method().clone();
    let body = axum::body::to_bytes(req.into_body(), 1 << 20)
        .await
        .unwrap_or_default();
    stub.hit(path.clone());
    if stub.failing.lock().contains(&path) {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if path == "/sat/manifest.json" {
        return axum::Json(stub.manifest.lock().clone()).into_response();
    }
    if let Some(rel) = path.strip_prefix("/sat/") {
        let Some((bytes, lm)) = stub.regions.lock().get(rel).cloned() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        if headers
            .get(header::IF_MODIFIED_SINCE)
            .and_then(|v| v.to_str().ok())
            == Some(lm.as_str())
        {
            return StatusCode::NOT_MODIFIED.into_response();
        }
        return (
            [
                (header::CONTENT_TYPE, "image/png".to_string()),
                (header::LAST_MODIFIED, lm),
            ],
            bytes,
        )
            .into_response();
    }
    if path == "/wm/WorldManifest.json" {
        let Some((bytes, etag)) = stub.world.lock().clone() else {
            return StatusCode::FORBIDDEN.into_response();
        };
        if headers
            .get(header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            == Some(etag.as_str())
        {
            return StatusCode::NOT_MODIFIED.into_response();
        }
        return (
            [
                (header::CONTENT_TYPE, "application/json".to_string()),
                (header::ETAG, etag),
            ],
            bytes,
        )
            .into_response();
    }
    if let Some(rel) = path.strip_prefix("/bucket/") {
        return match stub.posters.lock().get(rel).cloned() {
            Some((bytes, ct)) => ([(header::CONTENT_TYPE, ct)], bytes).into_response(),
            None => (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "application/xml")],
                "<Error/>",
            )
                .into_response(),
        };
    }
    if path == "/rpc" && method == axum::http::Method::POST {
        if *stub.rpc_down.lock() {
            return StatusCode::BAD_GATEWAY.into_response();
        }
        let call: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let data = call["params"][0]["data"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let result = match data.get(..10) {
            Some("0xc87b56dd") => stub
                .token_uris
                .lock()
                .get(&data[10..])
                .map(|u| abi_string(u))
                .unwrap_or_else(|| "0x".into()),
            _ => "0x".into(),
        };
        return axum::Json(json!({"jsonrpc": "2.0", "id": 1, "result": result})).into_response();
    }
    if let Some(name) = path.strip_prefix("/meta/") {
        return match stub.metadata.lock().get(name).cloned() {
            Some(v) => axum::Json(v).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        };
    }
    if let Some(rest) = path.strip_prefix("/fallback/api/v2/chain/") {
        let id = rest.rsplit('/').next().unwrap_or_default().to_string();
        return match stub.fallback.lock().get(&id).cloned() {
            Some((status, v)) => {
                (StatusCode::from_u16(status).unwrap(), axum::Json(v)).into_response()
            }
            None => (
                StatusCode::NOT_FOUND,
                axum::Json(json!({"errors": ["not found upstream"]})),
            )
                .into_response(),
        };
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn serve_stub(stub: Arc<Stub>) -> SocketAddr {
    let app = Router::new().fallback(stub_handler).with_state(stub);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn settings_at(addr: SocketAddr) -> Settings {
    let base = format!("http://{addr}");
    let mut s = Settings::for_tests(Sources::defaults());
    s.satellite_upstream = Some(format!("{base}/sat"));
    s.world_manifest_upstream = format!("{base}/wm/WorldManifest.json");
    s.poster_upstream = Some(format!("{base}/bucket"));
    s.opensea.fallback = Some(format!("{base}/fallback"));
    s.opensea.rpc_ethereum = format!("{base}/rpc");
    s.opensea.rpc_polygon = format!("{base}/rpc");
    s.opensea.cryptokitties_api = format!("{base}/kitties");
    s.opensea.ipfs_gateways = vec![format!("{base}/ipfs-a/"), format!("{base}/ipfs-b/")];
    s
}

struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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

/// A node over `pool` and a store under `dir` (shared by a restarted node).
async fn node_at(settings: Settings, pool: PgPool, dir: &std::path::Path) -> Shared {
    let storage = Arc::new(Storage::from_config(dir).await.unwrap());
    let content = Arc::new(ContentState::new(
        pool.clone(),
        crate::content::Settings::for_tests(),
        storage,
    ));
    State::new(settings, pool, crate::http::Outbound::for_tests(), content)
        .await
        .unwrap()
}

fn tmp_dir() -> TempDir {
    TempDir(std::env::temp_dir().join(format!(
        "catalyst-statics-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    )))
}

async fn call(
    app: &Router,
    path: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMap, Bytes) {
    let mut req = axum::http::Request::builder().uri(path);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let resp = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    (
        status,
        headers,
        axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
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

async fn pass_row(
    pool: &PgPool,
    mirror: &str,
) -> (
    i64,
    Option<chrono::NaiveDateTime>,
    Option<chrono::NaiveDateTime>,
    Option<String>,
) {
    sqlx::query_as("SELECT passes, started_at, completed_at, last_error FROM explore_api.mirror_passes WHERE mirror = $1")
        .bind(mirror)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The pass clock at the database's precision (microseconds), so a stamp
/// read back compares equal to the one written.
fn now() -> chrono::NaiveDateTime {
    let t = chrono::Utc::now().naive_utc();
    t - chrono::Duration::nanoseconds(i64::from(t.and_utc().timestamp_subsec_nanos() % 1000))
}

fn pixel(png: &[u8], x: u32, y: u32) -> [u8; 4] {
    let pm = Pixmap::decode_png(png).unwrap();
    let i = ((y * pm.width() + x) * 4) as usize;
    pm.data()[i..i + 4].try_into().unwrap()
}

/// A region render: 4 px per parcel, one flat colour.
fn region_png(rgba: [u8; 4]) -> Vec<u8> {
    let mut pm = Pixmap::new(68, 68).unwrap();
    for p in pm.data_mut().as_chunks_mut::<4>().0 {
        p.copy_from_slice(&rgba);
    }
    pm.encode_png().unwrap()
}

const LM: &str = "Tue, 22 Sep 2026 11:16:59 GMT";

#[tokio::test]
async fn satellite_pass_mirrors_the_listed_regions_rechecks_conditionally_and_serves_tiles() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let stub = Arc::new(Stub::default());
    *stub.manifest.lock() = json!([[0, 0], [17, 0]]);
    stub.regions.lock().insert(
        "0/0.png".into(),
        (region_png([200, 10, 10, 255]), LM.into()),
    );
    stub.regions.lock().insert(
        "0/0.t.png".into(),
        (region_png([10, 200, 10, 255]), LM.into()),
    );
    stub.regions.lock().insert(
        "17/0.png".into(),
        (region_png([10, 10, 200, 255]), LM.into()),
    );
    let addr = serve_stub(stub.clone()).await;
    let dir = tmp_dir();
    let node = node_at(settings_at(addr), db.pool.clone(), &dir.0).await;
    let app = router(node.clone());
    let (jobs, ctx) = job_ctx().await;

    let (status, headers, empty) = call(&app, "/satellite/9/256/255.png", &[]).await;
    assert_eq!(
        (status, headers["content-type"].to_str().unwrap()),
        (StatusCode::OK, "image/png")
    );
    assert_eq!(
        pixel(&empty, 128, 128)[3],
        0,
        "no region and no grid: a transparent tile"
    );

    satellite::run_pass(&node, &ctx, now()).await.unwrap();
    assert_eq!(stub.hits("/sat/manifest.json"), 1);
    assert_eq!(
        (
            stub.hits("/sat/0/0.png"),
            stub.hits("/sat/0/0.t.png"),
            stub.hits("/sat/17/0.png"),
            stub.hits("/sat/17/0.t.png")
        ),
        (1, 1, 1, 1)
    );
    let rows: Vec<(i32, i32, String, i64, i32)> =
        sqlx::query_as("SELECT cx, cy, variant, byte_size, width FROM explore.satellite_region ORDER BY cx, cy, variant").fetch_all(&db.pool).await.unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| (r.0, r.1, r.2.as_str(), r.4))
            .collect::<Vec<_>>(),
        vec![(0, 0, "full", 68), (0, 0, "thumb", 68), (17, 0, "full", 68)]
    );
    let (passes, started, completed, err) = pass_row(&db.pool, satellite::MIRROR).await;
    assert!(passes == 1 && completed >= started && err.is_none());
    let cid: String = sqlx::query_scalar("SELECT blob_cid FROM explore.satellite_region WHERE cx = 0 AND cy = 0 AND variant = 'full'").fetch_one(&db.pool).await.unwrap();
    assert!(
        node.content.storage.exist(&cid).await.unwrap(),
        "the render is in the ONE store"
    );

    // `.jpg` is the suffix /about advertises for the satellite view.
    for path in [
        "/satellite/9/256/255.png",
        "/satellite/9/256/255.jpg",
        "/api/satellite/9/256/255",
    ] {
        let (status, headers, png) = call(&app, path, &[]).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(
            headers["cache-control"],
            "public, max-age=3600, stale-while-revalidate=3600"
        );
        assert_eq!(
            pixel(&png, 128, 128),
            [200, 10, 10, 255],
            "{path}: zoom 9 samples the full render"
        );
    }
    let (_, _, low) = call(&app, "/satellite/4/8/7", &[]).await;
    assert_eq!(
        pixel(&low, 0, 255),
        [10, 200, 10, 255],
        "zoom 4 samples the thumbnail"
    );
    let (_, _, east) = call(&app, "/satellite/9/273/255", &[]).await;
    assert_eq!(
        pixel(&east, 128, 128),
        [10, 10, 200, 255],
        "region 17,0 covers parcel 17,0"
    );
    assert_eq!(
        call(&app, "/satellite/10/0/0", &[]).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, "/satellite/3/0/nope.png", &[]).await.0,
        StatusCode::BAD_REQUEST
    );

    // An unchanged upstream answers 304 to every region; no blob moves.
    let gen_before = node.satellite.generation();
    satellite::run_pass(&node, &ctx, now()).await.unwrap();
    assert_eq!(
        (stub.hits("/sat/0/0.png"), stub.hits("/sat/17/0.png")),
        (2, 2)
    );
    assert_eq!(
        node.satellite.generation(),
        gen_before,
        "a pass of 304s keeps the generation"
    );
    let fetched_twice: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM explore.satellite_region WHERE fetched_at < checked_at",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(fetched_twice, 3, "checked_at moved, fetched_at did not");

    // A new render replaces the blob, moves the generation and the tile.
    stub.regions.lock().insert(
        "0/0.png".into(),
        (
            region_png([250, 250, 0, 255]),
            "Wed, 23 Sep 2026 00:00:00 GMT".into(),
        ),
    );
    satellite::run_pass(&node, &ctx, now()).await.unwrap();
    assert!(node.satellite.generation() > gen_before);
    let (_, _, png) = call(&app, "/satellite/9/256/255", &[]).await;
    assert_eq!(
        pixel(&png, 128, 128),
        [250, 250, 0, 255],
        "the output cache does not serve the old render"
    );

    // A restarted node serves from the persisted index before any pass.
    let restarted = node_at(settings_at(addr), db.pool.clone(), &dir.0).await;
    assert_eq!(restarted.satellite.region_count(), 2);
    let (_, _, png) = call(&router(restarted), "/satellite/9/273/255", &[]).await;
    assert_eq!(pixel(&png, 128, 128), [10, 10, 200, 255]);

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

#[tokio::test]
async fn a_failed_satellite_pass_resumes_from_its_stamps_and_pruning_has_a_fuse() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let stub = Arc::new(Stub::default());
    let regions: Vec<(i32, i32)> = (0..6).map(|i| (i * 17, 0)).collect();
    *stub.manifest.lock() = json!(
        regions
            .iter()
            .map(|(x, y)| json!([x, y]))
            .collect::<Vec<_>>()
    );
    for (x, y) in &regions {
        stub.regions.lock().insert(
            format!("{x}/{y}.png"),
            (region_png([1, 2, 3, 255]), LM.into()),
        );
    }
    stub.failing.lock().insert("/sat/51/0.png".into());
    let addr = serve_stub(stub.clone()).await;
    let dir = tmp_dir();
    // The operator's opt-in for a tailnet render rig: the unguarded path.
    let mut settings = settings_at(addr);
    settings.satellite_allow_private = true;
    let node = node_at(settings, db.pool.clone(), &dir.0).await;
    let (jobs, ctx) = job_ctx().await;

    let t0 = now();
    let err = satellite::run_pass(&node, &ctx, t0)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("1 region fetches failed"), "{err}");
    let (passes, started, completed, last_error) = pass_row(&db.pool, satellite::MIRROR).await;
    assert_eq!((passes, completed), (0, None));
    assert!(last_error.unwrap().contains("51,0 full"));
    let held: i64 = sqlx::query_scalar("SELECT count(*) FROM explore.satellite_region")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(held, 5, "every other region landed before the failure");
    assert_eq!(
        node.satellite.region_count(),
        5,
        "the index serves what landed"
    );

    // The resumed pass keeps its start, skips every region stamped since and
    // fetches only the one that failed (its thumbnail is probed as well).
    stub.failing.lock().clear();
    let before: HashMap<String, usize> = stub.hits.lock().clone();
    satellite::run_pass(&node, &ctx, t0 + chrono::Duration::seconds(5))
        .await
        .unwrap();
    let (passes, started2, completed, _) = pass_row(&db.pool, satellite::MIRROR).await;
    assert_eq!(started2, started, "the resumed pass keeps its started_at");
    assert!(passes == 1 && completed >= started2);
    for (x, y) in &regions {
        let path = format!("/sat/{x}/{y}.png");
        let delta = stub.hits(&path) - before.get(&path).copied().unwrap_or(0);
        assert_eq!(delta, usize::from(*x == 51), "{path}");
    }
    assert_eq!(node.satellite.region_count(), 6);

    // An incomplete pass past the resume window starts afresh.
    stub.failing.lock().insert("/sat/0/0.png".into());
    let t1 = now() + chrono::Duration::seconds(10);
    assert!(satellite::run_pass(&node, &ctx, t1).await.is_err());
    stub.failing.lock().clear();
    let late = t1 + chrono::Duration::days(30);
    satellite::run_pass(&node, &ctx, late).await.unwrap();
    let (_, started3, _, _) = pass_row(&db.pool, satellite::MIRROR).await;
    assert_eq!(started3, Some(late));

    // A truncated manifest (fewer than half the held regions) prunes nothing;
    // a manifest that dropped one region prunes it.
    *stub.manifest.lock() = json!([[0, 0], [17, 0]]);
    satellite::run_pass(&node, &ctx, now() + chrono::Duration::days(31))
        .await
        .unwrap();
    assert_eq!(node.satellite.region_count(), 6, "the fuse held");
    *stub.manifest.lock() = json!(
        regions
            .iter()
            .take(5)
            .map(|(x, y)| json!([x, y]))
            .collect::<Vec<_>>()
    );
    satellite::run_pass(&node, &ctx, now() + chrono::Duration::days(32))
        .await
        .unwrap();
    assert_eq!(node.satellite.region_count(), 5);
    let pruned: i64 =
        sqlx::query_scalar("SELECT count(*) FROM explore.satellite_region WHERE cx = 85")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(pruned, 0);

    // An empty or unreachable manifest keeps everything.
    *stub.manifest.lock() = json!([]);
    assert!(
        satellite::run_pass(&node, &ctx, now() + chrono::Duration::days(33))
            .await
            .is_err()
    );
    stub.failing.lock().insert("/sat/manifest.json".into());
    assert!(
        satellite::run_pass(&node, &ctx, now() + chrono::Duration::days(34))
            .await
            .is_err()
    );
    assert_eq!(node.satellite.region_count(), 5);

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

#[tokio::test]
async fn world_manifest_is_mirrored_verbatim_revalidated_and_never_replaced_by_a_bad_body() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let stub = Arc::new(Stub::default());
    let good = br#"{"occupied":[[0,0],[1,0]],"roads":[[2,0]],"empty":[[3,0]]}"#.to_vec();
    *stub.world.lock() = Some((good.clone(), "\"v1\"".into()));
    let addr = serve_stub(stub.clone()).await;
    let dir = tmp_dir();
    let node = node_at(settings_at(addr), db.pool.clone(), &dir.0).await;
    let app = router(node.clone());
    let (jobs, ctx) = job_ctx().await;

    let (status, headers, body) = call(&app, "/world-manifest.json", &[]).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(headers["retry-after"], "60");
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        (v["source"].as_str(), v["configure"].as_str()),
        (Some("world-manifest"), Some("EXPLORER_MIRRORS_ENABLED"))
    );

    world_manifest::run_pass(&node, &ctx).await.unwrap();
    let (status, headers, body) = call(&app, "/world-manifest.json", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_ref(), good.as_slice(), "served byte for byte");
    assert_eq!(
        (
            headers["content-type"].to_str().unwrap(),
            headers["cache-control"].to_str().unwrap()
        ),
        ("application/json", "max-age=86400")
    );
    assert!(headers.contains_key("expires") && headers.contains_key("last-modified"));
    let etag = headers["etag"].to_str().unwrap().to_string();
    assert_eq!(
        call(&app, "/world-manifest.json", &[("if-none-match", &etag)])
            .await
            .0,
        StatusCode::NOT_MODIFIED
    );
    let row: (i32, i32, i32, Option<String>) = sqlx::query_as(
        "SELECT occupied_count, roads_count, empty_count, etag FROM explore.world_manifest",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(row, (2, 1, 1, Some("\"v1\"".into())));

    // Unchanged upstream: the held etag makes it a 304.
    let checked_before: chrono::NaiveDateTime =
        sqlx::query_scalar("SELECT checked_at FROM explore.world_manifest")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    world_manifest::run_pass(&node, &ctx).await.unwrap();
    let (checked_after, fetched_at): (chrono::NaiveDateTime, chrono::NaiveDateTime) =
        sqlx::query_as("SELECT checked_at, fetched_at FROM explore.world_manifest")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(checked_after > checked_before && fetched_at < checked_after);

    // Refused bodies keep the held copy and fail the pass.
    for (bad, tag) in [
        (&br#"{"occupied":[],"roads":[[1,1]]}"#[..], "\"v2\""),
        (br#"{"occupied":"x"}"#, "\"v3\""),
        (b"<html>", "\"v4\""),
    ] {
        *stub.world.lock() = Some((bad.to_vec(), tag.into()));
        assert!(world_manifest::run_pass(&node, &ctx).await.is_err());
        let (_, _, body) = call(&app, "/world-manifest.json", &[]).await;
        assert_eq!(body.as_ref(), good.as_slice());
    }
    let (_, _, _, err) = pass_row(&db.pool, world_manifest::MIRROR).await;
    assert!(
        err.unwrap().contains("not"),
        "the refusal is recorded on the marker"
    );

    // A restarted node serves the persisted copy before its first pass.
    let restarted = node_at(settings_at(addr), db.pool.clone(), &dir.0).await;
    let (status, _, body) = call(&router(restarted), "/world-manifest.json", &[]).await;
    assert_eq!((status, body.as_ref()), (StatusCode::OK, good.as_slice()));

    jobs.shutdown(Duration::from_secs(1)).await;
    db.teardown().await;
}

#[tokio::test]
async fn posters_read_through_once_keep_the_bucket_type_and_remember_misses() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let stub = Arc::new(Stub::default());
    let jpeg = [&[0xff, 0xd8, 0xff, 0xe0][..], b"....JFIF-body"].concat();
    let webp = b"RIFF\x10\x00\x00\x00WEBPVP8 body".to_vec();
    stub.posters.lock().insert(
        "poster/d2dfb36b6a012314.jpg".into(),
        (jpeg.clone(), "application/octet-stream".into()),
    );
    stub.posters.lock().insert(
        "poster-vertical/85c9f954-9529-453a-9777-90778bf46f94.webp".into(),
        (webp.clone(), "image/webp".into()),
    );
    stub.posters.lock().insert(
        "poster/evil.png".into(),
        (b"<html><script>".to_vec(), "text/html".into()),
    );
    let addr = serve_stub(stub.clone()).await;
    let dir = tmp_dir();
    let node = node_at(settings_at(addr), db.pool.clone(), &dir.0).await;
    let app = router(node.clone());

    for path in [
        "/events-assets-099ac00/poster/d2dfb36b6a012314.jpg",
        "/poster/d2dfb36b6a012314.jpg",
    ] {
        let (status, headers, body) = call(&app, path, &[]).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(body.as_ref(), jpeg.as_slice());
        assert_eq!(
            headers["content-type"], "application/octet-stream",
            "the bucket's type, as the old proxy passed it"
        );
        assert_eq!(
            headers["cache-control"],
            "public, max-age=31536000, immutable"
        );
        assert_eq!(headers["x-content-type-options"], "nosniff");
    }
    assert_eq!(
        stub.hits("/bucket/poster/d2dfb36b6a012314.jpg"),
        1,
        "read through once, then the index answers"
    );
    let (status, headers, body) = call(
        &app,
        "/events-assets-099ac00/poster-vertical/85c9f954-9529-453a-9777-90778bf46f94.webp",
        &[],
    )
    .await;
    assert_eq!(
        (
            status,
            headers["content-type"].to_str().unwrap(),
            body.as_ref()
        ),
        (StatusCode::OK, "image/webp", webp.as_slice())
    );
    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT kind, content_type, origin FROM explore.poster ORDER BY kind")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        rows,
        vec![
            (
                "poster".into(),
                "application/octet-stream".into(),
                "upstream".into()
            ),
            (
                "poster-vertical".into(),
                "image/webp".into(),
                "upstream".into()
            )
        ]
    );

    // Not an image: refused, nothing indexed.
    assert_eq!(
        call(&app, "/poster/evil.png", &[]).await.0,
        StatusCode::BAD_GATEWAY
    );
    let evil: i64 =
        sqlx::query_scalar("SELECT count(*) FROM explore.poster WHERE file = 'evil.png'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(evil, 0);

    // An upstream miss is a 404, remembered: the second ask stays local.
    for _ in 0..2 {
        let (status, _, body) = call(&app, "/poster/absent.jpg", &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({"ok": false, "error": "Not found"})
        );
    }
    assert_eq!(stub.hits("/bucket/poster/absent.jpg"), 1);
    for bad in [
        "/poster/..%2Fetc",
        "/poster/.hidden",
        "/events-assets-099ac00/poster/a%20b.jpg",
    ] {
        assert_eq!(call(&app, bad, &[]).await.0, StatusCode::NOT_FOUND, "{bad}");
    }
    assert_eq!(stub.hits_prefixed("/bucket/poster/."), 0);

    // Without an upstream (the events catalogue not mirrored) the index alone answers.
    let mut local_only = settings_at(addr);
    local_only.poster_upstream = None;
    let quiet = node_at(local_only, db.pool.clone(), &dir.0).await;
    let app2 = router(quiet);
    assert_eq!(
        call(&app2, "/poster/d2dfb36b6a012314.jpg", &[]).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app2, "/poster/other.jpg", &[]).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(stub.hits("/bucket/poster/other.jpg"), 0);

    db.teardown().await;
}

const CONTRACT: &str = "0x1111111111111111111111111111111111111111";

#[tokio::test]
async fn opensea_resolves_on_chain_caches_falls_back_and_serves_stale() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let stub = Arc::new(Stub::default());
    let addr = serve_stub(stub.clone()).await;
    stub.token_uris.lock().insert(
        opensea::id_hex("7").unwrap(),
        format!("http://{addr}/meta/7.json"),
    );
    stub.token_uris
        .lock()
        .insert(opensea::id_hex("8").unwrap(), "ipfs://QmMeta8".into());
    stub.metadata.lock().insert("7.json".into(), json!({"name": "Seven", "description": "d", "image": "ipfs://QmImg7", "animation_url": "ar://anim"}));
    stub.fallback.lock().insert(
        "9".into(),
        (
            200,
            json!({"nft": {"identifier": "9", "name": "From OpenSea"}}),
        ),
    );
    let dir = tmp_dir();
    let node = node_at(settings_at(addr), db.pool.clone(), &dir.0).await;
    let app = router(node.clone());

    let (status, headers, body) = call(
        &app,
        &format!(
            "/opensea/api/v2/chain/ethereum/contract/{}/nfts/7",
            CONTRACT.to_uppercase().replace("0X", "0x")
        ),
        &[],
    )
    .await;
    assert_eq!(
        (status, headers["content-type"].to_str().unwrap()),
        (StatusCode::OK, "application/json")
    );
    let v: Value = serde_json::from_slice(&body).unwrap();
    let base = format!("http://{addr}");
    assert_eq!(v["nft"]["name"], "Seven");
    assert_eq!(v["nft"]["description"], "d");
    assert_eq!(v["nft"]["contract"], CONTRACT);
    assert_eq!(v["nft"]["token_standard"], "erc721");
    assert_eq!(
        v["nft"]["image_url"],
        format!("{base}/ipfs-a/QmImg7"),
        "an http metadata URL renders ipfs images through the first gateway"
    );
    assert_eq!(v["nft"]["animation_url"], "https://arweave.net/anim");
    assert_eq!(v["nft"]["metadata_url"], format!("{base}/meta/7.json"));
    let source: String =
        sqlx::query_scalar("SELECT source FROM explore.opensea_nft WHERE token_id = '7'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(source, "chain");
    let rpc_hits = stub.hits("/rpc");
    let (status, _, again) = call(
        &app,
        &format!("/opensea/api/v2/chain/ethereum/contract/{CONTRACT}/nfts/7"),
        &[],
    )
    .await;
    assert_eq!(
        (status, again),
        (StatusCode::OK, body.clone()),
        "the cached text, byte for byte"
    );
    assert_eq!(
        stub.hits("/rpc"),
        rpc_hits,
        "a fresh row asks nothing upstream"
    );

    // ipfs:// metadata walks the gateways in order; the one that answered
    // renders the images.
    let (status, _, body) = call(
        &app,
        &format!("/opensea/api/v2/chain/polygon/contract/{CONTRACT}/nfts/8"),
        &[],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "neither stub gateway serves /ipfs-*/QmMeta8, and the fallback has no 8"
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"errors": ["not found upstream"]}),
        "the fallback's 404 body"
    );
    assert_eq!(
        (stub.hits("/ipfs-a/QmMeta8"), stub.hits("/ipfs-b/QmMeta8")),
        (1, 1)
    );
    let fb = stub.hits_prefixed("/fallback/");
    assert_eq!(
        call(
            &app,
            &format!("/opensea/api/v2/chain/polygon/contract/{CONTRACT}/nfts/8"),
            &[]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        stub.hits_prefixed("/fallback/"),
        fb,
        "the fallback's 404 is remembered"
    );

    // Not resolvable here: the fallback's 200 is served and cached as such.
    let (status, _, body) = call(
        &app,
        &format!("/opensea/api/v2/chain/matic/contract/{CONTRACT}/nfts/9"),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["nft"]["name"],
        "From OpenSea"
    );
    let source: String =
        sqlx::query_scalar("SELECT source FROM explore.opensea_nft WHERE token_id = '9'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(source, "fallback");

    // A stale row whose refresh fails is served as it was.
    sqlx::query("UPDATE explore.opensea_nft SET resolved_at = resolved_at - interval '31 days' WHERE token_id = '7'").execute(&db.pool).await.unwrap();
    *stub.rpc_down.lock() = true;
    stub.fallback
        .lock()
        .insert("7".into(), (503, json!({"errors": ["busy"]})));
    let (status, _, stale) = call(
        &app,
        &format!("/opensea/api/v2/chain/ethereum/contract/{CONTRACT}/nfts/7"),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<Value>(&stale).unwrap()["nft"]["name"],
        "Seven"
    );
    // ... and a stale row whose refresh succeeds is replaced.
    *stub.rpc_down.lock() = false;
    stub.metadata
        .lock()
        .insert("7.json".into(), json!({"name": "Seven v2"}));
    let (_, _, fresh) = call(
        &app,
        &format!("/opensea/api/v2/chain/ethereum/contract/{CONTRACT}/nfts/7"),
        &[],
    )
    .await;
    assert_eq!(
        serde_json::from_slice::<Value>(&fresh).unwrap()["nft"]["name"],
        "Seven v2"
    );

    // The expiry job drops rows past twice the freshness window.
    sqlx::query("UPDATE explore.opensea_nft SET resolved_at = resolved_at - interval '61 days' WHERE token_id = '9'").execute(&db.pool).await.unwrap();
    assert_eq!(opensea::expire(&db.pool, now()).await.unwrap(), 1);
    let left: Vec<String> =
        sqlx::query_scalar("SELECT token_id FROM explore.opensea_nft ORDER BY token_id")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(left, vec!["7".to_string()]);

    // The probes and the old unknown-route answer.
    let (status, _, body) = call(&app, "/opensea/health", &[]).await;
    assert_eq!(
        (status, serde_json::from_slice::<Value>(&body).unwrap()),
        (StatusCode::OK, json!({"ok": true}))
    );
    for bad in [
        format!("/opensea/api/v2/chain/bsc/contract/{CONTRACT}/nfts/1"),
        format!("/opensea/api/v2/chain/ethereum/contract/{CONTRACT}/nfts/x1"),
        "/opensea/nope".to_string(),
    ] {
        let (status, _, body) = call(&app, &bad, &[]).await;
        assert_eq!(
            (status, serde_json::from_slice::<Value>(&body).unwrap()),
            (StatusCode::NOT_FOUND, json!({"errors": ["unknown route"]})),
            "{bad}"
        );
    }

    db.teardown().await;
}

#[tokio::test]
async fn opensea_answers_land_estates_and_items_from_this_node_first() {
    let Some(db) = scratch_database().await else {
        return;
    };
    let stub = Arc::new(Stub::default());
    let addr = serve_stub(stub.clone()).await;
    let dir = tmp_dir();
    let node = node_at(settings_at(addr), db.pool.clone(), &dir.0).await;
    let tile = |x: i32, y: i32, estate: Option<&str>, name: Option<&str>| {
        let mut t = json!({"id": format!("{x},{y}"), "x": x, "y": y, "type": "owned", "top": false, "left": false, "topLeft": false, "updatedAt": 1});
        if let Some(e) = estate {
            t["estateId"] = json!(e);
        }
        if let Some(n) = name {
            t["name"] = json!(n);
        }
        (
            format!("{x},{y}"),
            serde_json::from_value::<map::tiles::Tile>(t).unwrap(),
        )
    };
    node.map.set(map::Grid::from_tiles(
        [
            tile(-3, 7, None, Some("Corner")),
            tile(5, 5, Some("42"), Some("Estate Town")),
            tile(5, 6, Some("42"), None),
        ]
        .into_iter()
        .collect(),
        1,
        None,
        now(),
    ));
    let app = router(node.clone());
    let land = node.settings.land_contract_address.clone();
    let estate = node.settings.estate_contract_address.clone();
    let land_id = map::meta::encode_land_token_id(-3, 7);

    let (status, _, body) = call(
        &app,
        &format!("/opensea/api/v2/chain/ethereum/contract/{land}/nfts/{land_id}"),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["nft"]["name"], "Corner");
    assert!(
        v["nft"]["image_url"]
            .as_str()
            .unwrap()
            .ends_with("/parcels/-3/7/map.png?size=24&width=1024&height=1024")
    );
    assert_eq!(
        v["nft"]["metadata_url"],
        format!("http://localhost/api/v2/contracts/{land}/tokens/{land_id}")
    );
    let (_, _, body) = call(
        &app,
        &format!("/opensea/api/v2/chain/ethereum/contract/{estate}/nfts/42"),
        &[],
    )
    .await;
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["nft"]["name"],
        "Estate Town"
    );

    sqlx::query(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity,
             total_supply, max_supply, available, price, image, name, category, created_at, updated_at, reviewed_at, mirrored_at)
         VALUES ('matic', $1, 3, 137, 'wearable_v2', 'urn:decentraland:matic:collections-v2:0x11:3', '0x00', 'rare',
             1, 10, 9, 0, 'https://peer.decentraland.org/lambdas/collections/contents/urn/thumbnail', 'Hat', 'hat', 0, 0, 0, timezone('UTC', now()))",
    )
    .bind(CONTRACT)
    .execute(&db.pool)
    .await
    .unwrap();
    let token = (alloy_primitives::U256::from(3u8) << 216) | alloy_primitives::U256::from(12u8);
    let (status, _, body) = call(
        &app,
        &format!("/opensea/api/v2/chain/matic/contract/{CONTRACT}/nfts/{token}"),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["nft"]["name"], "Hat");
    assert_eq!(
        v["nft"]["image_url"],
        "https://peer.decentraland.org/lambdas/collections/contents/urn/thumbnail"
    );
    assert_eq!(
        v["nft"]["metadata_url"],
        format!(
            "https://peer.decentraland.org/lambdas/collections/standard/erc721/137/{CONTRACT}/3/12"
        )
    );
    assert_eq!(
        v["nft"]["opensea_url"],
        format!("https://opensea.io/assets/matic/{CONTRACT}/{token}")
    );
    assert_eq!(stub.hits("/rpc"), 0, "the local legs ask no chain");
    let sources: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT source FROM explore.opensea_nft")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(sources, vec!["local".to_string()]);

    db.teardown().await;
}
