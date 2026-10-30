// from: catalyrst/crates/catalyrst-events/tests/mirror.rs, catalyrst/crates/catalyrst-events/tests/attendance_pg.rs, catalyrst/crates/catalyrst-places/tests/worlds_mirror_roundtrip.rs, catalyrst/crates/catalyrst-places/tests/places_mirror_retires_derived_pg.rs
//! The explore mirrors against a real postgres and a stub upstream: complete
//! passes, the two-miss sweep and its fuse, the local-overlay rules, the
//! unlisted-row confirmation, restart/resume from the persisted cursor, and
//! the read routes (worlds served in the upstream shape, events with the
//! viewer's attendance, `/api/status` with the pass markers). Needs
//! CATALYST_TEST_DATABASE_URL; each test gets its own scratch database.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::extract::Query;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Timelike, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

use catalyst::explore::events::mirror as events_mirror;
use catalyst::explore::events::{EventListFilters, EventListType, EventsComponent};
use catalyst::explore::places::mirror as places_mirror;
use catalyst::explore::places::mirror::{Leg, SweepOutcome};
use catalyst::explore::{ExploreState, MirrorSource, Settings, mirror_state, router};

#[path = "support/db.rs"]
mod db;

const WORLDS_PAGE: &str = include_str!("fixtures/explore_mirrors/worlds_page.json");
const ACTIVE_PAGE: &str =
    include_str!("fixtures/explore_mirrors/upstream_events_active_page0.json");
const HOUR: Duration = Duration::from_secs(3600);

fn fixture_worlds() -> Vec<Value> {
    let body: Value = serde_json::from_str(WORLDS_PAGE).unwrap();
    places_mirror::parse_page(&body).unwrap()
}

fn fixture_events() -> Vec<Value> {
    let body: Value = serde_json::from_str(ACTIVE_PAGE).unwrap();
    events_mirror::parse_page(&body).unwrap()
}

fn state(pool: &PgPool) -> Arc<ExploreState> {
    Arc::new(ExploreState::new(pool.clone(), Settings::for_tests(), None))
}

async fn get_json(pool: &PgPool, uri: &str) -> (StatusCode, Value) {
    let app = router(state(pool));
    let res = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .expect("route response");
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .expect("body");
    (status, serde_json::from_slice(&bytes).expect("json body"))
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_string()))
        .fetch_one(pool)
        .await
        .unwrap()
}

fn as_ts(v: &Value) -> Option<DateTime<Utc>> {
    v.as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&Utc))
}

fn json_eq(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

#[derive(serde::Deserialize)]
struct Q {
    offset: Option<i64>,
}

/// One canned answer of the stub upstream's detail endpoint for an unlisted id.
#[derive(Clone)]
enum Detail {
    Served(Value),
    NotFound,
}

/// Serves `pages` in order by offset (page size `page`), an empty page
/// beyond, and a detail endpoint answering from `details` (503 otherwise).
/// `fail_first_page` answers 500 at offset 0. `seen` records every offset asked.
struct Upstream {
    addr: SocketAddr,
    seen: Arc<std::sync::Mutex<Vec<i64>>>,
    hold: Arc<tokio::sync::Notify>,
}

async fn serve_upstream(
    route: &'static str,
    pages: Vec<Vec<Value>>,
    page: i64,
    fail_first_page: bool,
    details: Vec<(String, Detail)>,
    hold_at: Option<i64>,
) -> Upstream {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let hold = Arc::new(tokio::sync::Notify::new());
    let held = Arc::new(AtomicUsize::new(0));
    let details = Arc::new(details);
    let pages = Arc::new(pages);
    let total: usize = pages.iter().map(Vec::len).sum();
    let list = {
        let (seen, hold, held, pages) = (seen.clone(), hold.clone(), held.clone(), pages.clone());
        get(move |Query(q): Query<Q>| {
            let (seen, hold, held, pages) =
                (seen.clone(), hold.clone(), held.clone(), pages.clone());
            async move {
                let offset = q.offset.unwrap_or(0);
                seen.lock().unwrap().push(offset);
                if hold_at == Some(offset) && held.fetch_add(1, Ordering::SeqCst) == 0 {
                    hold.notify_one();
                    std::future::pending::<()>().await;
                }
                if offset == 0 && fail_first_page {
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
                let index = (offset / page) as usize;
                let data = pages.get(index).cloned().unwrap_or_default();
                Ok(Json(json!({"ok": true, "total": total, "data": data})))
            }
        })
    };
    let detail = get(
        move |axum::extract::Path(id): axum::extract::Path<String>| {
            let details = details.clone();
            async move {
                match details
                    .iter()
                    .find(|(known, _)| *known == id)
                    .map(|(_, d)| d)
                {
                    Some(Detail::Served(event)) => Ok(Json(json!({"ok": true, "data": event}))),
                    Some(Detail::NotFound) => Err(StatusCode::NOT_FOUND),
                    None => Err(StatusCode::SERVICE_UNAVAILABLE),
                }
            }
        },
    );
    let app = Router::new()
        .route(route, list)
        .route(&format!("{route}/{{id}}"), detail);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Upstream { addr, seen, hold }
}

impl Upstream {
    fn source(&self) -> MirrorSource {
        MirrorSource::unchecked(&format!("http://{}", self.addr))
    }

    fn offsets(&self) -> Vec<i64> {
        self.seen.lock().unwrap().clone()
    }
}

fn client() -> catalyst::http::Outbound {
    catalyst::http::Outbound::for_tests()
}

fn synthetic_world(i: usize) -> Value {
    let id = format!("w{i:03}.dcl.eth");
    json!({"id": id, "world_name": id, "title": format!("World {i}"), "image": "https://cdn.example/w.png"})
}

#[tokio::test]
async fn mirrored_worlds_serve_the_upstream_shape() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let rows = fixture_worlds();
    for row in &rows {
        assert!(
            places_mirror::upsert_world(&pool, row)
                .await
                .expect("upsert"),
            "{row}"
        );
    }
    let (status, body) = get_json(&pool, "/api/worlds?limit=100&offset=0").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], Value::Bool(true));
    assert_eq!(body["total"].as_i64(), Some(rows.len() as i64));
    let served = body["data"].as_array().expect("data array");
    assert_eq!(served.len(), rows.len());
    for upstream in &rows {
        let id = upstream["id"].as_str().unwrap();
        let world = served
            .iter()
            .find(|w| w["id"] == upstream["id"])
            .unwrap_or_else(|| panic!("{id} must be served"));
        for key in [
            "id",
            "title",
            "description",
            "image",
            "owner",
            "world_name",
            "content_rating",
            "categories",
            "likes",
            "dislikes",
            "favorites",
            "like_rate",
            "like_score",
            "disabled",
            "disabled_at",
            "base_position",
            "contact_name",
            "highlighted",
            "highlighted_image",
            "ranking",
            "is_private",
            "show_in_places",
            "single_player",
            "skybox_time",
            "user_visits",
            "user_count",
            "user_favorite",
            "user_like",
            "user_dislike",
        ] {
            assert!(
                json_eq(&world[key], &upstream[key]),
                "{id}: served {key} must equal upstream: {} != {}",
                world[key],
                upstream[key]
            );
        }
        for key in ["created_at", "updated_at", "deployed_at"] {
            assert_eq!(
                as_ts(&world[key]),
                as_ts(&upstream[key]),
                "{id}: served {key} must equal upstream"
            );
        }
        assert_eq!(
            world["world"],
            Value::Bool(true),
            "{id} must serve as a world"
        );
    }
    let (_, named) = get_json(&pool, "/api/worlds?names=metadynelabs.dcl.eth").await;
    assert_eq!(named["data"].as_array().map(Vec::len), Some(1));
    let (status, one) = get_json(&pool, "/api/worlds/MetadyneLabs.dcl.eth").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(one["data"]["id"], "metadynelabs.dcl.eth");
    let (status, missing) = get_json(&pool, "/api/worlds/nowhere.dcl.eth").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(missing["ok"], Value::Bool(false));
    let (_, names) = get_json(&pool, "/api/world_names").await;
    assert_eq!(names["total"].as_i64(), Some(5));
    let (_, destinations) = get_json(
        &pool,
        "/api/destinations?kinds=world&with=live_events,next_event",
    )
    .await;
    assert_eq!(destinations["total"].as_i64(), Some(5), "{destinations}");
    let first = &destinations["data"][0];
    assert_eq!(first["kind"], "world");
    assert_eq!(first["live"], Value::Bool(false));
    assert_eq!(first["next_event"], Value::Null);
    db.teardown().await;
}

#[tokio::test]
async fn worlds_sweep_deletes_unseen_worlds_but_spares_locals_and_places() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let rows = fixture_worlds();
    for row in &rows {
        places_mirror::upsert_world(&pool, row)
            .await
            .expect("upsert");
    }
    sqlx::query(
        "INSERT INTO explore.place (id, base_position, raw, fetched_at) \
         VALUES ('genesis', '1,1', '{}', timezone('UTC', now()) - interval '3 days')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO explore.place_world_local \
         (id, base_position, deployed_at, raw, world, world_name, fetched_at) \
         VALUES ('world:local.dcl.eth', '0,0', timezone('UTC', now()), \
                 '{\"world\": true, \"world_name\": \"local.dcl.eth\"}', true, 'local.dcl.eth', \
                 timezone('UTC', now()) - interval '3 days')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let dropped = rows[0]["id"].as_str().unwrap().to_string();
    let kept = rows[1]["id"].as_str().unwrap().to_string();
    let local = rows[2]["id"].as_str().unwrap().to_string();
    sqlx::query("UPDATE explore.place SET fetched_at = timezone('UTC', now()) - interval '3 hours' WHERE id = ANY($1)")
        .bind(vec![dropped.clone(), local.clone()])
        .execute(&pool)
        .await
        .unwrap();
    // This node's own row, however old, is never the mirror's to sweep.
    sqlx::query("UPDATE explore.place SET origin = 'local' WHERE id = $1")
        .bind(&local)
        .execute(&pool)
        .await
        .unwrap();
    // A wallet liked and favorited a mirrored world, and the one that goes.
    for id in [&dropped, &kept] {
        sqlx::query(
            "WITH f AS (INSERT INTO explore.user_favorites (\"user\", entity_id) VALUES ('0xa1', $1)), \
                  l AS (INSERT INTO explore.user_likes (\"user\", entity_id, \"like\") VALUES ('0xa1', $1, true)) \
             INSERT INTO explore.place_interaction_counts (entity_id, favorites, likes) VALUES ($1, 1, 1)",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let cutoff = Utc::now() - chrono::Duration::hours(2);
    assert_eq!(
        places_mirror::sweep_removed(&pool, Leg::Worlds, cutoff)
            .await
            .unwrap(),
        SweepOutcome::Swept(1)
    );
    for table in [
        "explore.user_favorites",
        "explore.user_likes",
        "explore.place_interaction_counts",
    ] {
        let ids: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT entity_id FROM {table}"
        )))
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            ids,
            [kept.as_str()],
            "{table}: the swept world's rows go with it"
        );
    }
    let (_, body) = get_json(&pool, "/api/worlds?limit=100&offset=0").await;
    let ids: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["id"].as_str().unwrap())
        .collect();
    assert!(!ids.contains(&dropped.as_str()), "{dropped} must be gone");
    for row in &rows[1..] {
        assert!(
            ids.contains(&row["id"].as_str().unwrap()),
            "refreshed worlds must survive the sweep"
        );
    }
    assert!(
        ids.contains(&"world:local.dcl.eth"),
        "place_world_local is outside the mirror's sweep"
    );
    assert!(
        ids.contains(&local.as_str()),
        "an origin 'local' row is outside the mirror's sweep"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM explore.place WHERE id = 'genesis'"
        )
        .await,
        1
    );
    db.teardown().await;
}

#[tokio::test]
async fn worlds_pass_pages_upserts_sweeps_twice_missed_and_records_a_complete_pass() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    sqlx::query(
        "INSERT INTO explore.place (id, base_position, raw, fetched_at) VALUES \
         ('missed-once.dcl.eth', '0,0', '{\"world\": true, \"world_name\": \"missed-once.dcl.eth\"}', timezone('UTC', now()) - interval '90 minutes'), \
         ('missed-twice.dcl.eth', '0,0', '{\"world\": true, \"world_name\": \"missed-twice.dcl.eth\"}', timezone('UTC', now()) - interval '3 hours')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let up = serve_upstream(
        "/api/worlds",
        vec![fixture_worlds()],
        100,
        false,
        vec![],
        None,
    )
    .await;
    let out = places_mirror::run_worlds_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(out.upserted, 5);
    assert_eq!(
        (out.skipped_rows, out.failed_rows, out.failed_pages),
        (0, 0, 0)
    );
    assert!(out.complete && !out.cancelled && out.resumed_from.is_none());
    assert_eq!(out.swept, 1, "missed twice is swept, missed once survives");
    assert_eq!(
        count(&pool, "SELECT count(*) FROM explore.place WHERE world").await,
        6
    );
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM explore.place WHERE id IN ('missed-once.dcl.eth', 'missed-twice.dcl.eth')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(ids, vec!["missed-once.dcl.eth"]);

    let status = mirror_state::status(&pool).await.unwrap();
    let worlds = status
        .iter()
        .find(|m| m.mirror == "worlds")
        .expect("worlds marker");
    assert_eq!(worlds.in_flight_offset, None);
    assert_eq!(worlds.last_pass_complete, Some(true));
    assert_eq!(worlds.last_complete_pass_rows, Some(5));
    assert!(worlds.lag_secs.is_some_and(|l| l < 60), "{worlds:?}");
    db.teardown().await;
}

#[tokio::test]
async fn worlds_pass_fails_open_never_sweeps_and_reports_incomplete() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    sqlx::query(
        "INSERT INTO explore.place (id, base_position, raw, fetched_at) \
         VALUES ('stale.dcl.eth', '0,0', '{\"world\": true, \"world_name\": \"stale.dcl.eth\"}', timezone('UTC', now()) - interval '3 days')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let up = serve_upstream(
        "/api/worlds",
        vec![fixture_worlds()],
        100,
        true,
        vec![],
        None,
    )
    .await;
    let out = places_mirror::run_worlds_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(out.failed_pages >= 1);
    assert!(!out.complete);
    assert_eq!(out.swept, 0);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM explore.place WHERE id = 'stale.dcl.eth'"
        )
        .await,
        1
    );
    let status = mirror_state::status(&pool).await.unwrap();
    let worlds = status.iter().find(|m| m.mirror == "worlds").unwrap();
    assert_eq!(worlds.last_pass_complete, Some(false));
    assert_eq!(worlds.last_complete_pass_at, None);
    assert_eq!(
        worlds.lag_secs, None,
        "no complete pass means no freshness claim"
    );
    assert!(
        worlds
            .last_error
            .as_deref()
            .is_some_and(|e| e.contains("failed pages")),
        "{worlds:?}"
    );
    db.teardown().await;
}

#[tokio::test]
async fn sweep_fuse_refuses_mass_deletion_but_allows_small_sweeps() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    for row in &fixture_worlds() {
        places_mirror::upsert_world(&pool, row)
            .await
            .expect("upsert");
    }
    let synthetic: Vec<String> = (0..95).map(|i| format!("w{i:03}.dcl.eth")).collect();
    for id in &synthetic {
        places_mirror::upsert_world(&pool, &json!({"id": id, "world_name": id}))
            .await
            .expect("upsert synthetic world");
    }
    assert_eq!(
        count(&pool, "SELECT count(*) FROM explore.place WHERE world").await,
        100
    );
    sqlx::query("UPDATE explore.place SET fetched_at = timezone('UTC', now()) - interval '3 hours' WHERE id = ANY($1)")
        .bind(&synthetic[..60])
        .execute(&pool)
        .await
        .unwrap();
    let cutoff = Utc::now() - chrono::Duration::hours(2);
    assert_eq!(
        places_mirror::sweep_removed(&pool, Leg::Worlds, cutoff)
            .await
            .unwrap(),
        SweepOutcome::Refused {
            candidates: 60,
            mirrored: 100
        }
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM explore.place WHERE world").await,
        100
    );
    sqlx::query("UPDATE explore.place SET fetched_at = timezone('UTC', now()) WHERE id = ANY($1)")
        .bind(&synthetic[..50])
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        places_mirror::sweep_removed(&pool, Leg::Worlds, cutoff)
            .await
            .unwrap(),
        SweepOutcome::Swept(10)
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM explore.place WHERE world").await,
        90
    );
    db.teardown().await;
}

/// The restart/resume proof: shutdown cancels the worlds pass after the first
/// page; the cursor and pass start persist; the next run resumes at page 2
/// (never re-fetching page 1), completes, and the sweep honours the original
/// pass start.
#[tokio::test]
async fn a_cancelled_worlds_pass_resumes_from_its_persisted_cursor() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let pages: Vec<Vec<Value>> = vec![
        (0..100).map(synthetic_world).collect(),
        (100..200).map(synthetic_world).collect(),
        (200..250).map(synthetic_world).collect(),
    ];
    let up = serve_upstream("/api/worlds", pages, 100, false, vec![], Some(100)).await;
    let cancel = CancellationToken::new();
    let run = {
        let (pool, source, cancel) = (pool.clone(), up.source(), cancel.clone());
        tokio::spawn(async move {
            places_mirror::run_worlds_pass(&client(), &pool, &source, HOUR, &cancel).await
        })
    };
    up.hold.notified().await;
    cancel.cancel();
    let out = run.await.unwrap().unwrap();
    assert!(out.cancelled && !out.complete);
    assert_eq!(out.upserted, 100);
    let status = mirror_state::status(&pool).await.unwrap();
    let marker = status.iter().find(|m| m.mirror == "worlds").unwrap();
    assert_eq!(
        marker.in_flight_offset,
        Some(100),
        "the cursor survives the interruption"
    );
    assert_eq!(marker.last_complete_pass_at, None);
    let started = marker.pass_started_at.expect("pass start persisted");

    let out = places_mirror::run_worlds_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(out.resumed_from, Some(100));
    assert!(out.complete);
    assert_eq!(
        out.upserted, 250,
        "the resumed pass counts the rows before the kill"
    );
    let offsets = up.offsets();
    assert_eq!(
        offsets,
        vec![0, 100, 100, 200],
        "page 1 is never fetched twice: {offsets:?}"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM explore.place WHERE world").await,
        250
    );
    let status = mirror_state::status(&pool).await.unwrap();
    let marker = status.iter().find(|m| m.mirror == "worlds").unwrap();
    assert_eq!(marker.in_flight_offset, None);
    assert_eq!(
        marker.pass_started_at,
        Some(started),
        "the resumed pass keeps its original start"
    );
    assert_eq!(marker.last_complete_pass_rows, Some(250));
    assert!(marker.lag_secs.is_some());
    db.teardown().await;
}

const UPSTREAM_PLAZA: &str = "3f0d2c1e-0000-4000-8000-000000000001";
const DERIVED_PLAZA: &str = "derived-0-0";
const WORLD: &str = "harbour.dcl.eth";

async fn seed_places(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO explore.place (id, base_position, title, raw) VALUES \
         ($1, '0,0', 'Genesis Plaza', \
          jsonb_build_object('source', 'content', 'world', false, 'positions', jsonb_build_array('0,0'))), \
         ($2, '0,0', 'Harbour', \
          jsonb_build_object('world', true, 'world_name', $2::text))",
    )
    .bind(DERIVED_PLAZA)
    .bind(WORLD)
    .execute(pool)
    .await
    .expect("seed a derived place and a world");
}

/// Makes a fixture event one created on this node, as `create_as` writes it.
async fn created_here(pool: &PgPool, id: &str) {
    sqlx::query(
        "INSERT INTO explore.events_local (id, signer, signed_payload) VALUES ($1, '0x00', '{\"create\": {}}')",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE explore.event SET origin = 'local' WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

async fn place_ids(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT id FROM explore.place ORDER BY id")
        .fetch_all(pool)
        .await
        .expect("read place ids")
}

#[tokio::test]
async fn a_complete_places_pass_replaces_the_derived_rows_keeps_worlds_and_serves_them() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    seed_places(&pool).await;
    let plaza = json!({
        "id": UPSTREAM_PLAZA,
        "title": "Genesis Plaza",
        "description": "Welcome <link=\"javascript:alert(1)\">here</link>",
        "image": "https://cdn.example/plaza.png",
        "owner": "0xABC",
        "base_position": "0,0",
        "positions": ["0,0", "0,1"],
        "categories": ["poi"],
        "likes": 40,
        "highlighted": true,
        "world": false,
        "user_count": 7,
        "like_score": 0.9,
    });
    let up = serve_upstream("/api/places", vec![vec![plaza]], 100, false, vec![], None).await;
    let out = places_mirror::run_places_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .expect("mirror pass");
    assert_eq!((out.upserted, out.retired, out.swept), (1, 1, 0));
    assert!(out.complete);
    assert_eq!(place_ids(&pool).await, vec![UPSTREAM_PLAZA, WORLD]);

    let (status, body) = get_json(&pool, "/api/places?limit=10").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    let p = &body["data"][0];
    assert_eq!(p["id"], UPSTREAM_PLAZA);
    assert_eq!(
        p["description"], "Welcome here",
        "the sanitizer strips the unsafe link"
    );
    assert_eq!(p["positions"], json!(["0,0", "0,1"]));
    assert_eq!(p["owner"], "0xabc");
    assert_eq!(p["user_count"], 7);
    assert_eq!(p["like_score"], 0.9);
    let (status, one) = get_json(
        &pool,
        &format!("/api/places/{UPSTREAM_PLAZA}?with_realms_detail=true"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // A place with users lists this realm with its count, as upstream does.
    assert_eq!(
        one["data"]["realms_detail"],
        json!([{ "serverName": "main", "usersCount": 7 }])
    );
    let alias = catalyst::explore::places::parcel_place_id("0,0");
    let (status, aliased) = get_json(&pool, &format!("/api/places/{alias}")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a derive-era id resolves to the row serving the parcel"
    );
    assert_eq!(aliased["data"]["id"], UPSTREAM_PLAZA);
    let (_, map) = get_json(&pool, "/api/map?positions=0,0").await;
    assert_eq!(map["data"]["0,0"]["id"], UPSTREAM_PLAZA);
    let (_, categories) = get_json(&pool, "/api/categories?target=places").await;
    assert_eq!(categories["data"][0]["name"], "poi");
    assert_eq!(categories["data"][0]["count"], 1);
    let (_, feed) = get_json(&pool, "/api/destinations").await;
    assert_eq!(
        feed["total"], 1,
        "the content gate keeps the thumbnail-less world out of the generic feed: {feed}"
    );
    assert_eq!(feed["data"][0]["kind"], "place");
    let (_, named) = get_json(&pool, "/api/destinations?world_names=Harbour.dcl.eth").await;
    assert_eq!(
        named["total"], 1,
        "a name lookup is exempt from the gate: {named}"
    );
    assert_eq!(named["data"][0]["kind"], "world");
    db.teardown().await;
}

async fn get_json_with(state: Arc<ExploreState>, method: &str, uri: &str) -> (StatusCode, Value) {
    let res = router(state)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(if method == "POST" {
                    format!("[\"{UPSTREAM_PLAZA}\"]")
                } else {
                    String::new()
                }))
                .unwrap(),
        )
        .await
        .expect("route response");
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .expect("body");
    (status, serde_json::from_slice(&bytes).expect("json body"))
}

/// The descriptive rule: a configured places mirror with no complete pass
/// and no rows of this node's own has no copy (503); the places this node
/// derives from its content are live, listed or read alone, without a
/// stamp. Once a pass completes, its reads carry that pass's time as
/// `as_of`, and keep it after the mirror is switched off, while a place of
/// this node's own still reads live.
#[tokio::test]
async fn a_places_mirror_serves_its_copy_with_as_of_and_503_before_its_first_pass() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let plaza = json!({"id": UPSTREAM_PLAZA, "title": "Genesis Plaza", "base_position": "0,0", "positions": ["0,0"], "world": false});
    let up = serve_upstream("/api/places", vec![vec![plaza]], 100, false, vec![], None).await;
    let configured = || {
        let mut settings = Settings::for_tests();
        settings.places_upstream = Some(up.source());
        Arc::new(ExploreState::new(pool.clone(), settings, None))
    };
    let lists = [
        ("GET", "/api/places?limit=10"),
        ("POST", "/api/places"),
        ("GET", "/api/map?positions=0,0"),
        ("GET", "/api/destinations"),
        ("GET", "/v1/destinations"),
    ];

    for (method, uri) in lists.iter().copied().chain([
        ("GET", "/api/places/derived-0-0"),
        ("GET", "/v1/destinations/derived-0-0"),
    ]) {
        let (status, body) = get_json_with(configured(), method, uri).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {uri}: {body}"
        );
        assert_eq!(body["source"], "places-mirror", "{uri}");
        assert_eq!(body["configure"], "PLACES_UPSTREAM_URL", "{uri}");
    }
    seed_places(&pool).await;
    for (method, uri) in lists
        .iter()
        .copied()
        .chain([("GET", "/api/places/derived-0-0")])
    {
        let (status, body) = get_json_with(configured(), method, uri).await;
        assert_eq!(status, StatusCode::OK, "{method} {uri}: {body}");
        assert!(
            body.get("as_of").is_none(),
            "derived rows are live: {uri}: {body}"
        );
    }
    // The v1 reader resolves a destination its own way (and answers 404
    // for this bare seed row); the gate let it through unstamped.
    let (status, body) = get_json_with(configured(), "GET", "/v1/destinations/derived-0-0").await;
    assert_ne!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(body.get("as_of").is_none(), "{body}");
    let (status, worlds) = get_json_with(configured(), "GET", "/api/worlds").await;
    assert_eq!(status, StatusCode::OK, "no worlds mirror: live");
    assert!(worlds.get("as_of").is_none(), "{worlds}");
    let (status, events) = get_json_with(configured(), "GET", "/api/events").await;
    assert_eq!(status, StatusCode::OK);
    assert!(events.get("as_of").is_none(), "{events}");

    let out = places_mirror::run_places_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .expect("mirror pass");
    assert!(out.complete);
    let at = mirror_state::last_complete_pass(&pool, places_mirror::PLACES_MIRROR)
        .await
        .unwrap()
        .expect("a complete pass is on record")
        .and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    for (method, uri) in [
        ("GET", "/api/places?limit=10"),
        ("POST", "/api/places"),
        ("GET", "/api/map?positions=0,0"),
        ("GET", "/api/destinations"),
        ("GET", "/v1/destinations"),
    ] {
        let (status, body) = get_json_with(configured(), method, uri).await;
        assert_eq!(status, StatusCode::OK, "{method} {uri}: {body}");
        assert_eq!(body["as_of"], at.as_str(), "{method} {uri}");
        assert_eq!(
            body["stale_after_secs"],
            2 * Settings::for_tests().places_interval.as_secs(),
            "{method} {uri}"
        );
    }
    let (_, body) = get_json_with(configured(), "GET", "/api/places?limit=10").await;
    assert_eq!(
        body["data"][0]["id"], UPSTREAM_PLAZA,
        "the copy itself, unchanged"
    );
    let (_, off) = get_json(&pool, "/api/places?limit=10").await;
    assert_eq!(
        off["as_of"],
        at.as_str(),
        "the mirror switched off: still a copy"
    );
    sqlx::query(
        "INSERT INTO explore.place (id, base_position, title, raw) VALUES \
         ('own-5-5', '5,5', 'Own scene', \
          jsonb_build_object('source', 'content', 'local', true, 'world', false, 'positions', jsonb_build_array('5,5')))",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (status, own) = get_json_with(configured(), "GET", "/api/places/own-5-5").await;
    assert_eq!(status, StatusCode::OK, "{own}");
    assert!(own.get("as_of").is_none(), "this node's own scene: {own}");
    let (_, mirrored) = get_json_with(
        configured(),
        "GET",
        &format!("/api/places/{UPSTREAM_PLAZA}"),
    )
    .await;
    assert_eq!(mirrored["as_of"], at.as_str(), "{mirrored}");
    db.teardown().await;
}

/// Rows no mirror fed are live beside mirrors that have no copy yet: an
/// event written into this node and a world it hosts read without a stamp,
/// and their lists serve, while a mirrored event has no copy to serve.
#[tokio::test]
async fn an_event_written_here_and_a_world_hosted_here_are_live_before_any_pass() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let events = fixture_events();
    for e in &events[..2] {
        events_mirror::upsert_event(&pool, e).await.unwrap();
    }
    let mirrored = events[0]["id"].as_str().unwrap();
    let local = events[1]["id"].as_str().unwrap();
    let configured = || {
        let mut settings = Settings::for_tests();
        settings.events_upstream = Some(MirrorSource::unchecked("http://127.0.0.1:9/events"));
        settings.worlds_upstream = Some(MirrorSource::unchecked("http://127.0.0.1:9/worlds"));
        Arc::new(ExploreState::new(pool.clone(), settings, None))
    };
    let world = "hosted.dcl.eth";
    for uri in [
        "/api/events".to_string(),
        format!("/api/events/{local}"),
        "/api/worlds".to_string(),
        format!("/api/worlds/{world}"),
    ] {
        let (status, body) = get_json_with(configured(), "GET", &uri).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}: {body}");
    }

    created_here(&pool, local).await;
    for uri in [
        format!("/api/events/{local}"),
        format!("/v1/events/{local}"),
        "/api/events".to_string(),
    ] {
        let (status, body) = get_json_with(configured(), "GET", &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert!(body.get("as_of").is_none(), "{uri}: {body}");
    }
    let (status, body) =
        get_json_with(configured(), "GET", &format!("/api/events/{mirrored}")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "events-mirror");

    sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
        .bind(world)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get_json_with(configured(), "GET", &format!("/api/worlds/{world}")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "hosted here: not in the catalogue's local leg yet, but never 503: {body}"
    );
    sqlx::query(
        "INSERT INTO explore.place_world_local (id, base_position, title, raw, world, world_name) \
         VALUES ('hosted-world', '0,0', 'Hosted', '{}', true, $1)",
    )
    .bind(world)
    .execute(&pool)
    .await
    .unwrap();
    for uri in [format!("/api/worlds/{world}"), "/api/worlds".to_string()] {
        let (status, body) = get_json_with(configured(), "GET", &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert!(body.get("as_of").is_none(), "{uri}: {body}");
    }
    db.teardown().await;
}

/// A list a mirror with no complete pass serves on rows of this node's
/// own lists only those: what an interrupted first pass upserted stays out,
/// unstamped, until a pass completes and the copy is served, stamped.
#[tokio::test]
async fn a_list_before_the_first_complete_pass_serves_only_this_nodes_rows() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    // The derived plaza is this node's; the world beside it was mirrored.
    seed_places(&pool).await;
    let hosted = "hosted.dcl.eth";
    sqlx::query(
        "INSERT INTO explore.place_world_local (id, base_position, title, raw, world, world_name) \
         VALUES ('hosted-world', '0,0', 'Hosted', '{}', true, $1)",
    )
    .bind(hosted)
    .execute(&pool)
    .await
    .unwrap();
    // An interrupted first pass of each mirror: rows upserted, none completed.
    let upstream = "http://127.0.0.1:9";
    for mirror in [
        places_mirror::PLACES_MIRROR,
        places_mirror::WORLDS_MIRROR,
        events_mirror::EVENTS_MIRROR,
    ] {
        mirror_state::begin_pass(&pool, mirror, upstream)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO explore.place (id, base_position, title, raw) VALUES \
         ($1, '1,1', 'Upstream Plaza', jsonb_build_object('world', false, 'positions', jsonb_build_array('1,1')))",
    )
    .bind(UPSTREAM_PLAZA)
    .execute(&pool)
    .await
    .unwrap();
    let events = fixture_events();
    for e in &events[..2] {
        events_mirror::upsert_event(&pool, e).await.unwrap();
    }
    let mirrored = events[0]["id"].as_str().unwrap();
    let local = events[1]["id"].as_str().unwrap();
    created_here(&pool, local).await;
    let configured = || {
        let mut settings = Settings::for_tests();
        settings.places_upstream = Some(MirrorSource::unchecked(&format!("{upstream}/places")));
        settings.worlds_upstream = Some(MirrorSource::unchecked(&format!("{upstream}/worlds")));
        settings.events_upstream = Some(MirrorSource::unchecked(&format!("{upstream}/events")));
        Arc::new(ExploreState::new(pool.clone(), settings, None))
    };
    let ids = |body: &Value| -> Vec<String> {
        let mut ids: Vec<String> = body["data"]
            .as_array()
            .unwrap_or_else(|| panic!("a list: {body}"))
            .iter()
            .filter_map(|r| r["id"].as_str().map(String::from))
            .collect();
        ids.sort();
        ids
    };
    for (uri, want) in [
        ("/api/events?list=all&limit=500", vec![local.to_string()]),
        ("/api/places?limit=100", vec![DERIVED_PLAZA.to_string()]),
        ("/api/worlds?limit=100", vec!["hosted-world".to_string()]),
    ] {
        let (status, body) = get_json_with(configured(), "GET", uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert!(body.get("as_of").is_none(), "{uri}: {body}");
        assert_eq!(ids(&body), want, "{uri}: {body}");
    }
    let (status, names) = get_json_with(configured(), "GET", "/api/world_names").await;
    assert_eq!(
        (status, names["data"].clone()),
        (StatusCode::OK, json!([hosted])),
        "{names}"
    );
    for uri in ["/api/places", "/api/places/status"] {
        let res = router(configured())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!([UPSTREAM_PLAZA, DERIVED_PLAZA]).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("route response");
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
            .await
            .expect("body");
        let by_id: Value = serde_json::from_slice(&bytes).expect("json body");
        assert_eq!(status, StatusCode::OK, "{uri}: {by_id}");
        assert_eq!(
            ids(&by_id),
            vec![DERIVED_PLAZA.to_string()],
            "{uri}: {by_id}"
        );
    }

    // A completed events pass serves the copy, mirrored rows included.
    mirror_state::finish_pass(
        &pool,
        events_mirror::EVENTS_MIRROR,
        &mirror_state::PassEnd {
            complete: true,
            rows: 2,
            swept: 0,
            error: None,
        },
    )
    .await
    .unwrap();
    let (status, body) = get_json_with(configured(), "GET", "/api/events?list=all&limit=500").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.get("as_of").is_some(), "{body}");
    let mut both = vec![local.to_string(), mirrored.to_string()];
    both.sort();
    assert_eq!(ids(&body), both, "{body}");
    db.teardown().await;
}

#[tokio::test]
async fn an_empty_upstream_answer_leaves_the_derived_rows_in_place() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    seed_places(&pool).await;
    let up = serve_upstream("/api/places", vec![], 100, false, vec![], None).await;
    let out = places_mirror::run_places_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .expect("mirror pass");
    assert_eq!((out.upserted, out.retired), (0, 0));
    assert_eq!(place_ids(&pool).await, vec![DERIVED_PLAZA, WORLD]);
    db.teardown().await;
}

#[tokio::test]
async fn a_failed_places_page_keeps_the_cursor_and_the_next_run_resumes_there() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let up = serve_upstream("/api/places", vec![], 100, true, vec![], None).await;
    let err = places_mirror::run_places_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .expect_err("a failed page fails the strict places pass");
    assert!(err.to_string().contains("GET http://"), "{err}");
    let status = mirror_state::status(&pool).await.unwrap();
    let marker = status.iter().find(|m| m.mirror == "places").unwrap();
    assert_eq!(marker.in_flight_offset, Some(0));
    assert!(marker.last_error.is_some());
    assert_eq!(marker.last_complete_pass_at, None);
    db.teardown().await;
}

#[tokio::test]
async fn upsert_real_page_is_idempotent_and_serves() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let events = fixture_events();
    assert_eq!(events.len(), 100);
    for e in &events {
        assert!(events_mirror::upsert_event(&pool, e).await.unwrap());
    }
    assert_eq!(
        count(&pool, "SELECT count(*) FROM explore.event").await,
        100
    );
    for e in &events {
        assert!(events_mirror::upsert_event(&pool, e).await.unwrap());
    }
    assert_eq!(
        count(&pool, "SELECT count(*) FROM explore.event").await,
        100,
        "upsert is idempotent"
    );

    let component = EventsComponent::new(pool.clone(), None);
    let sample_id = events[0]["id"].as_str().unwrap();
    let rec = component
        .get(sample_id)
        .await
        .unwrap()
        .expect("mirrored event serves");
    assert_eq!(rec.id, sample_id);
    assert!(rec.approved);
    assert_eq!(rec.name, events[0]["name"].as_str().unwrap());
    assert_eq!(rec.x as i64, events[0]["coordinates"][0].as_i64().unwrap());
    assert_eq!(
        rec.start_at.map(|t| t.timestamp_millis()),
        as_ts(&events[0]["start_at"]).map(|t| t.timestamp_millis()),
        "a zoned upstream timestamp lands as the same instant"
    );
    let (all, total) = component
        .query(
            &EventListFilters {
                limit: 500,
                list: EventListType::All,
                ..Default::default()
            },
            true,
        )
        .await
        .unwrap();
    assert_eq!((total, all.len()), (100, 100));

    let (status, body) = get_json(&pool, "/api/events?list=all&limit=500").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().map(Vec::len), Some(100));
    let (status, one) = get_json(&pool, &format!("/api/events/{sample_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(one["data"]["id"], sample_id);
    assert!(one["data"]["attending"].is_boolean());
    let (status, none) = get_json(&pool, "/api/events/no-such-event").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(none["error"], "Not found event \"no-such-event\"");
    let (_, attendees) = get_json(&pool, &format!("/api/events/{sample_id}/attendees")).await;
    assert_eq!(
        attendees["data"].as_array().map(Vec::len),
        events[0]["latest_attendees"].as_array().map(Vec::len)
    );
    let (_, categories) = get_json(&pool, "/api/events/categories").await;
    assert_eq!(categories["data"].as_array().map(Vec::len), Some(25));
    let (_, schedules) = get_json(&pool, "/api/schedules").await;
    assert_eq!(schedules["data"], json!([]));
    db.teardown().await;
}

/// Upstream's cron leaves `next_start_at` alone until that occurrence ends,
/// so an occurrence the creator's edit dropped from `recurrent_dates` keeps
/// running (live, first in the list) instead of jumping to the next recurrence.
#[tokio::test]
async fn a_running_stored_occurrence_outranks_the_next_recurrence() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let now = Utc::now().with_nanosecond(0).unwrap();
    let day = chrono::Duration::days(1);
    let iso = |t: DateTime<Utc>| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let at = |v: &Value| as_ts(v).map(|t| t.timestamp());
    let event = |id: &str, started: DateTime<Utc>, dates: Vec<DateTime<Utc>>| {
        json!({
            "id": id, "name": id, "approved": true, "recurrent": true,
            "start_at": iso(started - day * 5), "finish_at": iso(started + day * 5),
            "duration": day.num_milliseconds(),
            "next_start_at": iso(started), "next_finish_at": iso(started + day),
            "recurrent_dates": dates.iter().map(|d| iso(*d)).collect::<Vec<_>>(),
            "coordinates": [0, 0], "user": "0xabc",
        })
    };
    let running = now - chrono::Duration::hours(12);
    let finished = now - day * 3;
    let events = [
        event("kept-running", running, vec![now + day, now + day * 2]),
        event(
            "moved-on",
            finished,
            vec![finished, now + day + chrono::Duration::hours(1)],
        ),
        event("last-one", finished, vec![finished - day, finished]),
    ];
    for e in &events {
        assert!(events_mirror::upsert_event(&pool, e).await.unwrap());
    }

    let (status, live) = get_json(&pool, "/api/events?list=live").await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = live["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["kept-running"]);
    assert_eq!(
        at(&live["data"][0]["next_start_at"]),
        Some(running.timestamp())
    );
    assert_eq!(
        at(&live["data"][0]["next_finish_at"]),
        Some((running + day).timestamp())
    );
    assert_eq!(live["data"][0]["live"], true);

    let (_, all) = get_json(&pool, "/api/events?list=all").await;
    let by_id = |id: &str| {
        all["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == id)
            .unwrap()
            .clone()
    };
    let ids: Vec<&str> = all["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["last-one", "kept-running", "moved-on"],
        "ordered by the effective next start"
    );
    assert_eq!(
        at(&by_id("moved-on")["next_start_at"]),
        Some((now + day + chrono::Duration::hours(1)).timestamp())
    );
    assert_eq!(by_id("moved-on")["live"], false);
    assert_eq!(
        at(&by_id("last-one")["next_start_at"]),
        Some(finished.timestamp()),
        "no unfinished recurrence: the last one"
    );
    db.teardown().await;
}

#[tokio::test]
async fn events_sweep_deletes_unseen_rows_and_their_rsvps_but_spares_local_events() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let events = fixture_events();
    for e in &events[..3] {
        events_mirror::upsert_event(&pool, e).await.unwrap();
    }
    let gone_id = events[0]["id"].as_str().unwrap();
    let local_id = events[1]["id"].as_str().unwrap();
    let fresh_id = events[2]["id"].as_str().unwrap();
    sqlx::query("UPDATE explore.event SET fetched_at = timezone('UTC', now()) - interval '3 hours' WHERE id = ANY($1)")
        .bind(vec![gone_id.to_string(), local_id.to_string()])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE explore.event SET origin = 'local', raw = raw || '{\"deleted_by_admin\": true}', name = 'locally deleted' WHERE id = $1",
    )
    .bind(local_id)
    .execute(&pool)
    .await
    .unwrap();
    // Two events a binary from before `origin` created (the default made them
    // 'upstream'): one by its create payload, one by an edit that kept the
    // creating transaction's signed_at.
    for (id, payload, at) in [
        ("legacy-created", json!({ "create": {} }), "3 hours"),
        ("legacy-edited", json!({ "edit": {} }), "4 hours"),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "WITH e AS ( \
                 INSERT INTO explore.event (id, name, raw, fetched_at) \
                 VALUES ($1, $1, '{{}}', timezone('UTC', now()) - interval '{at}') RETURNING fetched_at \
             ) INSERT INTO explore.events_local (id, signer, signed_payload, signed_at) \
               SELECT $1, '0x00000000000000000000000000000000000000a1', $2, \
                      CASE WHEN $2 ? 'create' THEN timezone('UTC', now()) ELSE e.fetched_at END FROM e"
        )))
        .bind(id)
        .bind(&payload)
        .execute(&pool)
        .await
        .unwrap();
    }
    // An operator's moderation of the unseen event, and of one already gone.
    sqlx::query(
        "INSERT INTO explore.event_moderation (event_id, hidden, set_by) \
         SELECT id, true, '0x00000000000000000000000000000000000000a1' FROM unnest($1::text[]) id",
    )
    .bind(vec![gone_id.to_string(), "already-gone".to_string()])
    .execute(&pool)
    .await
    .unwrap();
    // RSVPs: on each event, and on one the node no longer holds.
    sqlx::query(
        "INSERT INTO explore.event_attendance_local (event_id, signer, action) \
         SELECT id, '0x00000000000000000000000000000000000000a1', 'going' FROM unnest($1::text[]) id",
    )
    .bind(vec![
        gone_id.to_string(),
        local_id.to_string(),
        fresh_id.to_string(),
        "already-gone".to_string(),
    ])
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        events_mirror::upsert_event(&pool, &events[1])
            .await
            .unwrap()
    );
    let (name, raw): (String, Value) =
        sqlx::query_as("SELECT name, raw FROM explore.event WHERE id = $1")
            .bind(local_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        name, "locally deleted",
        "the mirror never overwrites a local signed change"
    );
    assert_eq!(raw["deleted_by_admin"], Value::Bool(true));

    let cutoff = Utc::now() - chrono::Duration::hours(2);
    assert_eq!(
        events_mirror::sweep_removed(&pool, cutoff).await.unwrap(),
        events_mirror::SweepOutcome::Swept(1)
    );
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM explore.event ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!ids.contains(&gone_id.to_string()), "unseen row deleted");
    assert!(ids.contains(&local_id.to_string()), "local write spared");
    assert!(ids.contains(&fresh_id.to_string()), "fresh row spared");
    let legacy: Vec<(String, String)> =
        sqlx::query_as("SELECT id, origin FROM explore.event WHERE id LIKE 'legacy-%' ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        legacy,
        [
            ("legacy-created".to_string(), "local".to_string()),
            ("legacy-edited".to_string(), "local".to_string())
        ],
        "an older binary's local events are reclassified, not swept"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM explore.event_moderation").await,
        0,
        "the swept event's moderation and the orphaned one go with it"
    );
    let mut rsvps: Vec<String> =
        sqlx::query_scalar("SELECT event_id FROM explore.event_attendance_local")
            .fetch_all(&pool)
            .await
            .unwrap();
    rsvps.sort();
    let mut kept = vec![local_id.to_string(), fresh_id.to_string()];
    kept.sort();
    assert_eq!(
        rsvps, kept,
        "the swept event's RSVPs and the orphaned ones go with it"
    );
    db.teardown().await;
}

/// The production case: an organiser deleted a recurring event upstream; the
/// public list dropped it at once while the detail kept serving it flagged
/// `deleted_by_user`. The mirror must hide it from our lists in the same pass
/// -- not list it for hours and then answer 404. A row whose detail is gone
/// outright is hidden the same way; only the grace sweep deletes it, with its
/// RSVPs. An event an older binary created is neither.
#[tokio::test]
async fn an_unlisted_row_is_settled_by_the_upstream_detail_in_the_same_pass() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let events = fixture_events();
    let deleted_id = "feaf0aaf-0f35-4c75-8a7b-3910a5c13194";
    let gone_id = "c80da832-ad7f-499d-a720-984cfcacfb60";
    let mut deleted_event = events[0].clone();
    deleted_event["id"] = Value::String(deleted_id.to_string());
    let mut gone_event = events[1].clone();
    gone_event["id"] = Value::String(gone_id.to_string());
    events_mirror::upsert_event(&pool, &deleted_event)
        .await
        .unwrap();
    events_mirror::upsert_event(&pool, &gone_event)
        .await
        .unwrap();
    sqlx::query("UPDATE explore.event SET fetched_at = timezone('UTC', now()) - interval '30 minutes' WHERE id = ANY($1)")
        .bind(vec![deleted_id.to_string(), gone_id.to_string()])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO explore.event_attendance_local (event_id, signer, action) \
         SELECT id, '0x00000000000000000000000000000000000000a1', 'going' FROM unnest($1::text[]) id",
    )
    .bind(vec![deleted_id.to_string(), gone_id.to_string()])
    .execute(&pool)
    .await
    .unwrap();
    // created by a binary from before `origin`: the default made it 'upstream'
    let legacy_id = "legacy-created";
    sqlx::query(
        "WITH e AS ( \
             INSERT INTO explore.event (id, name, raw, fetched_at) \
             VALUES ($1, $1, '{}', timezone('UTC', now()) - interval '30 minutes') RETURNING id \
         ) INSERT INTO explore.events_local (id, signer, signed_payload) \
           SELECT id, '0x00000000000000000000000000000000000000a1', '{\"create\": {}}' FROM e",
    )
    .bind(legacy_id)
    .execute(&pool)
    .await
    .unwrap();

    let listed = |pool: PgPool| async move {
        let (_, body) = get_json(&pool, "/api/events?list=all&limit=500").await;
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == deleted_id)
    };
    assert!(
        listed(pool.clone()).await,
        "the deleted event is still listed before the pass"
    );

    let mut upstream_copy = deleted_event.clone();
    upstream_copy["deleted_by_user"] = Value::Bool(true);
    upstream_copy["deleted_at"] = Value::String("2026-09-22T15:07:49.649Z".to_string());
    let up = serve_upstream(
        "/api/events",
        vec![events.clone()],
        500,
        false,
        vec![
            (deleted_id.to_string(), Detail::Served(upstream_copy)),
            (gone_id.to_string(), Detail::NotFound),
            (legacy_id.to_string(), Detail::NotFound),
        ],
        None,
    )
    .await;
    let out = events_mirror::run_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(out.complete);
    assert_eq!(
        (
            out.unlisted_refreshed,
            out.unlisted_gone,
            out.unlisted_unconfirmed
        ),
        (1, 1, 0)
    );
    assert_eq!(out.swept, 0, "nothing was old enough for the grace sweep");
    assert!(
        !listed(pool.clone()).await,
        "the deleted event left the public list in one pass"
    );
    let (raw,): (Value,) = sqlx::query_as("SELECT raw FROM explore.event WHERE id = $1")
        .bind(deleted_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(raw["deleted_by_user"], Value::Bool(true));
    let (raw, origin): (Value, String) =
        sqlx::query_as("SELECT raw, origin FROM explore.event WHERE id = $1")
            .bind(gone_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (raw["deleted_by_user"].clone(), origin.as_str()),
        (Value::Bool(true), "upstream"),
        "a detail 404 hides the row and leaves it to the grace sweep"
    );
    let (status, _) = get_json(&pool, &format!("/api/events/{gone_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (raw, origin): (Value, String) =
        sqlx::query_as("SELECT raw, origin FROM explore.event WHERE id = $1")
            .bind(legacy_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (raw.get("deleted_by_user"), origin.as_str()),
        (None, "local"),
        "an older binary's local event is reclassified, never asked upstream"
    );
    let rsvps = || {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT event_id FROM explore.event_attendance_local ORDER BY event_id",
            )
            .fetch_all(&pool)
            .await
            .unwrap()
        }
    };
    let mut both = vec![deleted_id.to_string(), gone_id.to_string()];
    both.sort();
    assert_eq!(rsvps().await, both, "hiding keeps the RSVPs");

    // Two intervals on, the guarded sweep deletes the gone row and its RSVP;
    // the refreshed rows and the local one stay.
    assert_eq!(
        events_mirror::sweep_removed(&pool, Utc::now() - chrono::Duration::minutes(10))
            .await
            .unwrap(),
        events_mirror::SweepOutcome::Swept(1)
    );
    let gone: Option<String> = sqlx::query_scalar("SELECT id FROM explore.event WHERE id = $1")
        .bind(gone_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert!(gone.is_none());
    assert_eq!(
        rsvps().await,
        [deleted_id.to_string()],
        "the gone event's RSVP left with it; the soft-deleted one's stays"
    );
    let listed_id = events[2]["id"].as_str().unwrap();
    let refused = catalyst::explore::events::moderation::moderate(
        &pool,
        listed_id,
        &catalyst::explore::events::moderation::AdminPatch {
            name: Some("renamed here".into()),
            ..Default::default()
        },
        "0x00000000000000000000000000000000000000a1",
    )
    .await
    .expect_err("a mirrored event's own fields refuse moderation");
    assert_eq!(
        refused.to_string(),
        format!(
            "The event {listed_id} is mirrored from {} and read-only on this node; edit it there",
            up.source().base()
        ),
        "the 409 names the upstream the pass read"
    );
    let (status, one) = get_json(&pool, &format!("/api/events/{deleted_id}")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a soft-deleted event is invisible to an anonymous viewer"
    );
    assert_eq!(one["ok"], Value::Bool(false));
    db.teardown().await;
}

/// Rows a detail 404 already hid keep their old `fetched_at` until the grace
/// sweep; more of them than the confirm slots must not keep a newly
/// unlisted event from being asked (and hidden) in its first pass.
#[tokio::test]
async fn rows_already_gone_never_starve_a_newly_unlisted_one() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let events = fixture_events();
    let gone: Vec<String> = (0..101).map(|i| format!("already-gone-{i:03}")).collect();
    sqlx::query(
        "INSERT INTO explore.event (id, name, raw, fetched_at) \
         SELECT id, id, '{\"deleted_by_user\": true, \"deleted_at\": \"2026-09-01T00:00:00.000Z\"}', \
                timezone('UTC', now()) - interval '90 minutes' \
         FROM unnest($1::text[]) id",
    )
    .bind(&gone)
    .execute(&pool)
    .await
    .unwrap();
    let new_id = "c3f1d7a2-5b8e-4c1d-9f2a-6e7b8c9d0e1f";
    let mut new_event = events[0].clone();
    new_event["id"] = Value::String(new_id.to_string());
    events_mirror::upsert_event(&pool, &new_event)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE explore.event SET fetched_at = timezone('UTC', now()) - interval '30 minutes' WHERE id = $1",
    )
    .bind(new_id)
    .execute(&pool)
    .await
    .unwrap();
    let mut upstream_copy = new_event.clone();
    upstream_copy["deleted_by_user"] = Value::Bool(true);
    let mut details: Vec<(String, Detail)> = gone
        .iter()
        .map(|id| (id.clone(), Detail::NotFound))
        .collect();
    details.push((new_id.to_string(), Detail::Served(upstream_copy)));
    let up = serve_upstream(
        "/api/events",
        vec![events.clone()],
        500,
        false,
        details,
        None,
    )
    .await;
    let out = events_mirror::run_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(out.complete);
    assert_eq!(
        (
            out.unlisted_refreshed,
            out.unlisted_gone,
            out.unlisted_unconfirmed
        ),
        (1, 99, 0),
        "the newly unlisted row takes a slot first; the gone rows fill the rest"
    );
    let (raw,): (Value,) = sqlx::query_as("SELECT raw FROM explore.event WHERE id = $1")
        .bind(new_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(raw["deleted_by_user"], Value::Bool(true));
    db.teardown().await;
}

#[tokio::test]
async fn events_pass_pages_upserts_sweeps_twice_missed_and_a_bad_page_never_sweeps() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    sqlx::query(
        "INSERT INTO explore.event (id, name, raw, fetched_at) VALUES \
         ('missed-once', 'missed-once', '{}', timezone('UTC', now()) - interval '90 minutes'), \
         ('missed-twice', 'missed-twice', '{}', timezone('UTC', now()) - interval '3 hours')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let up = serve_upstream(
        "/api/events",
        vec![fixture_events()],
        500,
        false,
        vec![],
        None,
    )
    .await;
    let out = events_mirror::run_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(out.upserted, 100);
    assert_eq!((out.failed_pages, out.failed_rows), (0, 0));
    assert!(out.complete);
    assert_eq!(out.swept, 1);
    assert_eq!(
        out.unlisted_unconfirmed, 2,
        "both unlisted rows were asked for and the stub could not settle them"
    );
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM explore.event WHERE id IN ('missed-once', 'missed-twice')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(ids, vec!["missed-once"], "one missed pass never deletes");
    assert_eq!(
        count(&pool, "SELECT count(*) FROM explore.event").await,
        101
    );

    let (status, body) = get_json(&pool, "/api/status").await;
    assert_eq!(status, StatusCode::OK);
    let events = body["data"]["mirrors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mirror"] == "events")
        .expect("events marker");
    assert_eq!(events["last_pass_complete"], Value::Bool(true));
    assert_eq!(events["last_complete_pass_rows"], 100);
    assert!(
        events["lag_secs"].as_i64().is_some_and(|l| l < 60),
        "{events}"
    );
    assert_eq!(events["in_flight_offset"], Value::Null);
    assert_eq!(body["data"]["catalogue"]["events"], 101);
    assert_eq!(
        body["data"]["configured_mirrors"]["events"],
        Value::Null,
        "a test node mirrors nothing by config"
    );

    sqlx::query(
        "INSERT INTO explore.event (id, name, raw, fetched_at) \
         VALUES ('stale', 'stale', '{}', timezone('UTC', now()) - interval '3 days')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let bad = serve_upstream(
        "/api/events",
        vec![fixture_events()],
        500,
        true,
        vec![],
        None,
    )
    .await;
    let out = events_mirror::run_pass(
        &client(),
        &pool,
        &bad.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(out.failed_pages >= 1 && !out.complete);
    assert_eq!(out.swept, 0);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM explore.event WHERE id = 'stale'"
        )
        .await,
        1,
        "an incomplete pass never deletes"
    );
    let (_, body) = get_json(&pool, "/api/status").await;
    let events = body["data"]["mirrors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mirror"] == "events")
        .unwrap();
    assert_eq!(events["last_pass_complete"], Value::Bool(false));
    assert_eq!(
        events["last_complete_pass_rows"], 100,
        "the last complete pass is remembered across a failed one"
    );
    db.teardown().await;
}

#[tokio::test]
async fn a_cancelled_events_pass_resumes_from_its_persisted_cursor() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    // Upstream pages are 500 events; the fixture holds 100, so the first page is
    // five copies under distinct ids and the second page 40 more.
    let events = fixture_events();
    let first: Vec<Value> = (0..5)
        .flat_map(|copy| {
            events.iter().enumerate().map(move |(i, e)| {
                let mut e = e.clone();
                e["id"] = Value::String(format!("first-page-{copy}-{i}"));
                e
            })
        })
        .collect();
    let mut second: Vec<Value> = events.iter().take(40).cloned().collect();
    for (i, e) in second.iter_mut().enumerate() {
        e["id"] = Value::String(format!("second-page-{i}"));
    }
    let up = serve_upstream(
        "/api/events",
        vec![first, second],
        500,
        false,
        vec![],
        Some(500),
    )
    .await;
    let cancel = CancellationToken::new();
    let run = {
        let (pool, source, cancel) = (pool.clone(), up.source(), cancel.clone());
        tokio::spawn(async move {
            events_mirror::run_pass(&client(), &pool, &source, HOUR, &cancel).await
        })
    };
    up.hold.notified().await;
    cancel.cancel();
    let out = run.await.unwrap().unwrap();
    assert!(out.cancelled && !out.complete);
    assert_eq!(out.upserted, 500);
    let marker = mirror_state::status(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.mirror == "events")
        .unwrap();
    assert_eq!(marker.in_flight_offset, Some(500));

    let out = events_mirror::run_pass(
        &client(),
        &pool,
        &up.source(),
        HOUR,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(out.resumed_from, Some(500));
    assert!(out.complete);
    assert_eq!(out.upserted, 540);
    assert_eq!(up.offsets(), vec![0, 500, 500]);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM explore.event").await,
        540
    );
    db.teardown().await;
}

const USER: &str = "0x000000000000000000000000000000000000BEEF";
const OTHER: &str = "0x000000000000000000000000000000000000cafe";

#[tokio::test]
async fn the_viewers_attendance_rides_in_the_list_and_get_statements() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let mut events = fixture_events();
    // Attendance filtering uses the current time. Keep the captured upstream
    // payload intact on disk, but give this test an occurrence that cannot age out.
    let occurrence = |event: &mut Value, start: DateTime<Utc>| {
        let finish = start + chrono::Duration::hours(2);
        event["start_at"] = json!(start);
        event["next_start_at"] = json!(start);
        event["finish_at"] = json!(finish);
        event["next_finish_at"] = json!(finish);
        event["duration"] = json!(7_200_000);
        event["recurrent"] = json!(false);
        event["recurrent_dates"] = json!([]);
    };
    let now = Utc::now();
    occurrence(&mut events[0], now + chrono::Duration::days(1));
    for e in &events {
        events_mirror::upsert_event(&pool, e).await.unwrap();
    }
    let id = events[0]["id"].as_str().unwrap();
    let upstream_user = events[0]["latest_attendees"][0].as_str().unwrap();
    sqlx::query(
        "INSERT INTO explore.event_attendance_local (event_id, signer, signed_payload, action) \
         VALUES ($1, $2, '{\"user_name\": \"Beef\"}', 'going')",
    )
    .bind(id)
    .bind(USER.to_lowercase())
    .execute(&pool)
    .await
    .unwrap();
    let component = EventsComponent::new(pool.clone(), None);

    let filters = EventListFilters {
        limit: 200,
        list: EventListType::All,
        user: Some(USER.to_string()),
        ..Default::default()
    };
    let (page, _) = component.query(&filters, false).await.unwrap();
    assert!(
        page.iter().find(|e| e.id == id).unwrap().attending,
        "a local RSVP marks the viewer attending"
    );
    let other = EventListFilters {
        user: Some(OTHER.to_string()),
        ..filters.clone()
    };
    let (page, _) = component.query(&other, false).await.unwrap();
    assert!(!page.iter().find(|e| e.id == id).unwrap().attending);
    let only = EventListFilters {
        only_attendee: true,
        ..filters.clone()
    };
    let (page, _) = component.query(&only, false).await.unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].id, id);

    assert!(
        component
            .get_for_viewer(id, Some(USER))
            .await
            .unwrap()
            .unwrap()
            .attending
    );
    assert!(
        component
            .get_for_viewer(id, Some(&upstream_user.to_uppercase()))
            .await
            .unwrap()
            .unwrap()
            .attending,
        "an upstream attendee is attending too"
    );
    assert!(
        !component
            .get_for_viewer(id, Some(OTHER))
            .await
            .unwrap()
            .unwrap()
            .attending
    );
    assert!(
        !component
            .get_for_viewer(id, None)
            .await
            .unwrap()
            .unwrap()
            .attending
    );
    assert!(
        component
            .get_for_viewer("no-such-event", Some(USER))
            .await
            .unwrap()
            .is_none()
    );
    let attending = component.attending(USER).await.unwrap();
    assert!(attending.iter().any(|e| e.id == id));

    let (_, attendees) = get_json(&pool, &format!("/api/events/{id}/attendees")).await;
    let listed = attendees["data"].as_array().unwrap();
    assert_eq!(
        listed[0]["user"],
        USER.to_lowercase(),
        "the local RSVP sorts first"
    );
    assert_eq!(listed[0]["user_name"], "Beef");
    assert_eq!(
        listed.len(),
        events[0]["latest_attendees"].as_array().unwrap().len() + 1
    );

    let original_total = component.get(id).await.unwrap().unwrap().total_attendees;
    catalyst::explore::events::attendees::rsvp_cancel(&pool, id, upstream_user)
        .await
        .unwrap();
    let cancelled = component
        .get_for_viewer(id, Some(upstream_user))
        .await
        .unwrap()
        .unwrap();
    assert!(
        !cancelled.attending,
        "local cancellation overrides the upstream preview"
    );
    assert_eq!(cancelled.total_attendees, (original_total - 1).max(0));
    assert!(
        !cancelled
            .latest_attendees
            .iter()
            .any(|address| address.eq_ignore_ascii_case(upstream_user))
    );
    let cancelled_filter = EventListFilters {
        user: Some(upstream_user.to_uppercase()),
        only_attendee: true,
        ..filters.clone()
    };
    let (cancelled_page, _) = component.query(&cancelled_filter, false).await.unwrap();
    assert!(!cancelled_page.iter().any(|event| event.id == id));
    assert!(
        !component
            .attending(upstream_user)
            .await
            .unwrap()
            .iter()
            .any(|event| event.id == id)
    );
    events_mirror::upsert_event(&pool, &events[0])
        .await
        .unwrap();
    assert!(
        !component
            .get_for_viewer(id, Some(upstream_user))
            .await
            .unwrap()
            .unwrap()
            .attending,
        "a mirror refresh cannot erase explicit cancellation"
    );
    let cancelled_list = catalyst::explore::events::attendees::list_for_event(&pool, id)
        .await
        .unwrap();
    assert!(
        !cancelled_list
            .iter()
            .any(|record| record.user.eq_ignore_ascii_case(upstream_user))
    );
    catalyst::explore::events::attendees::rsvp_going(&pool, id, upstream_user, None)
        .await
        .unwrap();
    let rejoined = component
        .get_for_viewer(id, Some(upstream_user))
        .await
        .unwrap()
        .unwrap();
    assert!(rejoined.attending);
    assert_eq!(
        rejoined.total_attendees, original_total,
        "rejoining does not double-count a mirrored attendee"
    );

    let mut finished = events[0].clone();
    occurrence(&mut finished, now - chrono::Duration::days(1));
    events_mirror::upsert_event(&pool, &finished).await.unwrap();
    assert!(
        !component
            .attending(USER)
            .await
            .unwrap()
            .iter()
            .any(|e| e.id == id),
        "a finished event is excluded even when the viewer has an RSVP"
    );
    assert!(
        component
            .get_for_viewer(id, Some(USER))
            .await
            .unwrap()
            .unwrap()
            .attending,
        "finishing the event does not erase the viewer's RSVP"
    );

    sqlx::query("INSERT INTO explore.moderators (address, added_at) VALUES ($1, 0)")
        .bind(OTHER)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        component
            .viewer_is_moderator(&OTHER.to_uppercase())
            .await
            .unwrap()
    );
    assert!(!component.viewer_is_moderator(USER).await.unwrap());
    db.teardown().await;
}

/// The destinations feed's live-event and next-event decorations read the
/// mirrored events directly: a live event at a place names it, an upcoming
/// one at a world (matched by its server name) is the world's next event.
#[tokio::test]
async fn destinations_decorate_live_and_next_events_from_the_mirrored_catalogue() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let now = Utc::now();
    let iso = |t: DateTime<Utc>| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    sqlx::query(
        "INSERT INTO explore.place (id, base_position, title, raw) VALUES \
         ('place-1', '10,20', 'Plaza', '{\"world\": false, \"positions\": [\"10,20\"], \"image\": \"https://cdn.example/p.png\", \"owner\": \"0xabc\"}'), \
         ('my-world.dcl.eth', '0,0', 'My World', '{\"world\": true, \"world_name\": \"My-World.dcl.eth\", \"image\": \"https://cdn.example/w.png\", \"owner\": \"0xabc\"}')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let live = json!({
        "id": "ev-live", "name": "Movie Night", "approved": true, "place_id": "PLACE-1",
        "start_at": iso(now - chrono::Duration::minutes(30)), "finish_at": iso(now + chrono::Duration::minutes(90)),
        "next_start_at": iso(now - chrono::Duration::minutes(30)), "next_finish_at": iso(now + chrono::Duration::minutes(90)),
        "duration": 7_200_000, "coordinates": [10, 20]
    });
    let upcoming = json!({
        "id": "ev-next", "name": "World Party", "approved": true, "world": true, "server": "my-world.dcl.eth",
        "place_id": "upstream-uuid",
        "start_at": iso(now + chrono::Duration::days(1)), "finish_at": iso(now + chrono::Duration::days(1) + chrono::Duration::hours(2)),
        "next_start_at": iso(now + chrono::Duration::days(1)), "next_finish_at": iso(now + chrono::Duration::days(1) + chrono::Duration::hours(2)),
        "duration": 7_200_000, "coordinates": [0, 0]
    });
    let rejected = json!({
        "id": "ev-rejected", "name": "Hidden", "approved": true, "rejected": true, "place_id": "place-1",
        "start_at": iso(now - chrono::Duration::minutes(30)), "finish_at": iso(now + chrono::Duration::minutes(90)),
        "duration": 7_200_000, "coordinates": [10, 20]
    });
    for e in [&live, &upcoming, &rejected] {
        events_mirror::upsert_event(&pool, e).await.unwrap();
    }
    let (status, body) = get_json(&pool, "/api/destinations?with=live_events,next_event").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = body["data"].as_array().unwrap();
    let place = data
        .iter()
        .find(|d| d["id"] == "place-1")
        .expect("the place is in the feed");
    assert_eq!(place["live"], Value::Bool(true));
    assert_eq!(place["live_event_name"], "Movie Night");
    assert_eq!(
        place["next_event"],
        Value::Null,
        "a live event is not upcoming"
    );
    let world = data
        .iter()
        .find(|d| d["id"] == "my-world.dcl.eth")
        .expect("the world is in the feed");
    assert_eq!(world["live"], Value::Bool(false));
    assert_eq!(world["live_event_name"], Value::Null);
    assert_eq!(world["next_event"]["id"], "ev-next");
    assert_eq!(world["next_event"]["name"], "World Party");

    let (_, live_list) = get_json(&pool, "/api/events?list=live").await;
    let ids: Vec<&str> = live_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["ev-live"], "a rejected event never lists");
    let (_, by_place) = get_json(
        &pool,
        "/api/events?list=all&places_ids=place-1&places_ids=My-World.dcl.eth",
    )
    .await;
    assert_eq!(by_place["data"]["total"], 2, "{by_place}");
    db.teardown().await;
}

/// Opt-in (`cargo test --test explore_mirrors -- --ignored live_`): one bounded
/// live pass of the worlds mirror against places.decentraland.org (about 1.6k
/// worlds, 17 pages of 100), through the checked source and the real client,
/// then the routes over the mirrored rows and the status markers.
#[tokio::test]
#[ignore]
async fn live_worlds_pass_against_places_decentraland_org() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let pool = db.pool.clone();
    let source = MirrorSource::checked(
        "https://places.decentraland.org",
        &["http://127.0.0.1:5210"],
    )
    .unwrap();
    let client = catalyst::http::Outbound::new(catalyst::http::HttpClientCfg::serve(4)).unwrap();
    let started = std::time::Instant::now();
    let out =
        places_mirror::run_worlds_pass(&client, &pool, &source, HOUR, &CancellationToken::new())
            .await
            .unwrap();
    eprintln!("live worlds pass: {out:?} in {:?}", started.elapsed());
    assert!(out.complete, "{out:?}");
    assert!(out.upserted > 1000, "{out:?}");
    let mirrored = count(&pool, "SELECT count(*) FROM explore.place WHERE world").await;
    assert_eq!(mirrored, out.upserted as i64 - out.skipped_rows as i64);
    let (status, body) = get_json(&pool, "/api/worlds?limit=3&order_by=most_active").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"].as_i64(), Some(mirrored));
    eprintln!(
        "top worlds: {:?}",
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| (w["world_name"].clone(), w["user_count"].clone()))
            .collect::<Vec<_>>()
    );
    let (_, status_body) = get_json(&pool, "/api/status").await;
    eprintln!("status: {}", status_body["data"]["mirrors"]);
    let worlds = status_body["data"]["mirrors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mirror"] == "worlds")
        .unwrap();
    assert_eq!(worlds["last_pass_complete"], Value::Bool(true));
    assert!(worlds["lag_secs"].as_i64().is_some());
    db.teardown().await;
}
