//! The old crate's two database contracts (every table written in one
//! transaction, first-wins duplicates, nulls kept, a failed child insert
//! rolling the whole pass back, the memo serving the collector's commit) over
//! a fake [`Sources`], plus what v2 adds: the empty-archive envelope, the
//! route shapes after a pass, the gate on the job, the bounded retention
//! sweep and the daily rollup. DB-backed tests use the process's scratch
//! database (`CATALYST_TEST_DATABASE_URL`) under one lock (this module is the
//! only writer of `presence.*`, and "the latest snapshot" is a whole-table
//! fact), uuid-scope their keys and delete their snapshots; without the
//! variable they skip.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use super::collector::{self, SnapshotData, pass_clock, store_snapshot};
use super::model::{HotScene, Island, Peer};
use super::sources::{LiveData, Sources};
use super::*;
use crate::db::SCHEMA_SQL;

static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A pass clock the tables can hold back exactly (microseconds).
fn now() -> DateTime<Utc> {
    pass_clock(Utc::now()).and_utc()
}

#[derive(Default)]
struct FakeSources {
    peers: Vec<Peer>,
    islands: Vec<Island>,
    hot_scenes: Vec<HotScene>,
    scenes: HashMap<String, Option<Vec<String>>>,
    worlds: HashMap<String, Option<Vec<String>>>,
    live: Option<LiveData>,
}

#[async_trait]
impl Sources for FakeSources {
    async fn statistics(&self) -> anyhow::Result<super::sources::Statistics> {
        Ok(super::sources::Statistics {
            peers: self.peers.clone(),
            islands: self.islands.clone(),
            hot_scenes: self.hot_scenes.clone(),
        })
    }
    async fn scene_participants(
        &self,
        realm: &str,
        pointer: &str,
    ) -> anyhow::Result<Option<Vec<String>>> {
        assert_eq!(realm, "main");
        Ok(self.scenes.get(pointer).cloned().flatten())
    }
    async fn world_participants(&self, world: &str) -> anyhow::Result<Option<Vec<String>>> {
        Ok(self.worlds.get(world).cloned().flatten())
    }
    async fn worlds_live_data(&self) -> anyhow::Result<Option<LiveData>> {
        Ok(self.live.clone())
    }
}

fn peer(address: &str, parcel: Option<(i32, i32)>) -> Peer {
    Peer {
        address: address.into(),
        parcel_x: parcel.map(|p| p.0),
        parcel_y: parcel.map(|p| p.1),
        position_x: parcel.map(|p| p.0 as f64 * 16.0),
        position_y: None,
        position_z: parcel.map(|p| p.1 as f64 * 16.0),
        last_ping: Some(42),
    }
}

fn hot_scene(
    id: &str,
    name: Option<&str>,
    base: Option<(i32, i32)>,
    users: Option<i32>,
    parcels: i32,
) -> HotScene {
    HotScene {
        scene_id: id.into(),
        name: name.map(Into::into),
        base_x: base.map(|b| b.0),
        base_y: base.map(|b| b.1),
        users_count: users,
        parcel_count: parcels,
        creator: None,
        description: None,
    }
}

/// The old `sample()`: duplicate peer and pointer keys, nulls, an empty world.
fn sample(taken_at: DateTime<Utc>) -> SnapshotData {
    SnapshotData {
        taken_at: taken_at.naive_utc(),
        realm: "main".into(),
        peers: vec![
            peer("0xaa", Some((1, -2))),
            peer("0xbb", None),
            peer("0xaa", Some((3, 4))),
        ],
        islands: vec![Island {
            island_id: "I1".into(),
            peer_count: 2,
            max_peers: None,
            center_x: Some(1.5),
            center_y: None,
            center_z: Some(-3.25),
            radius: Some(100.0),
        }],
        hot_scenes: vec![
            hot_scene("s1", Some("Plaza"), Some((1, -2)), Some(2), 4),
            hot_scene("s2", None, None, None, 0),
        ],
        occupancy: vec![
            (
                "1,-2".into(),
                Some("Plaza".into()),
                vec!["0xaa".into(), "0xbb".into()],
            ),
            ("1,-2".into(), Some("Dup".into()), vec![]),
            ("7,7".into(), None, vec![]),
        ],
        world_rows: vec![
            ("alice.eth".into(), vec!["0xcc".into()], 1),
            ("empty.eth".into(), vec![], 3),
        ],
        worlds_live_total: Some(4),
    }
}

async fn state(sources: FakeSources, writes_enabled: bool) -> Option<SharedState> {
    let pool = crate::db::scratch::pool(4).await?;
    Some(Arc::new(PresenceState::new(
        pool,
        Settings::new(writes_enabled),
        Arc::new(sources),
    )))
}

/// A state over a pool that never connects: for what touches no row.
fn state_without_db(writes_enabled: bool) -> SharedState {
    state_without_db_with(Settings::new(writes_enabled))
}

fn state_without_db_with(settings: Settings) -> SharedState {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(500))
        .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
        .unwrap();
    Arc::new(PresenceState::new(
        pool,
        settings,
        Arc::new(FakeSources::default()),
    ))
}

async fn get(app: &Router, path: &str) -> (StatusCode, Option<String>, Value) {
    let res = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let retry_after = res
        .headers()
        .get(header::RETRY_AFTER)
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        retry_after,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn delete_snapshots(pool: &sqlx::PgPool, ids: &[i64]) {
    sqlx::query("DELETE FROM presence.snapshots WHERE id = ANY($1)")
        .bind(ids)
        .execute(pool)
        .await
        .unwrap();
}

async fn count_where(pool: &sqlx::PgPool, sql: &'static str, id: i64) -> i64 {
    sqlx::query_scalar(sql)
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn store_snapshot_writes_every_table_and_memo_serves_latest() {
    let _lock = DB_LOCK.lock().await;
    let Some(state) = state(FakeSources::default(), true).await else {
        return;
    };
    let pool = &state.pool;
    let before = state
        .queries
        .current_bundle()
        .await
        .unwrap()
        .current
        .as_ref()
        .map(|c| c.snapshot_id);

    let taken_at = now();
    let id = store_snapshot(pool, &sample(taken_at)).await.unwrap();

    let counts: (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM presence.peer_snapshots WHERE snapshot_id = $1), \
                (SELECT count(*) FROM presence.island_snapshots WHERE snapshot_id = $1), \
                (SELECT count(*) FROM presence.hot_scene_snapshots WHERE snapshot_id = $1), \
                (SELECT count(*) FROM presence.scene_occupancy WHERE snapshot_id = $1), \
                (SELECT count(*) FROM presence.world_membership WHERE snapshot_id = $1)",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        counts,
        (2, 1, 2, 2, 2),
        "ON CONFLICT DO NOTHING keeps first-wins per key"
    );

    let (parcel_x, position_z, last_ping): (Option<i32>, Option<f64>, Option<i64>) = sqlx::query_as(
        "SELECT parcel_x, position_z, last_ping FROM presence.peer_snapshots WHERE snapshot_id = $1 AND address = '0xaa'",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        (parcel_x, position_z, last_ping),
        (Some(1), Some(-32.0), Some(42))
    );

    let (name, addresses, count, realm, occ_taken_at): (
        Option<String>,
        Value,
        i32,
        String,
        chrono::NaiveDateTime,
    ) = sqlx::query_as(
        "SELECT scene_name, addresses, count, realm, taken_at FROM presence.scene_occupancy \
             WHERE snapshot_id = $1 AND pointer = '1,-2'",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(name.as_deref(), Some("Plaza"));
    assert_eq!(addresses, json!(["0xaa", "0xbb"]));
    assert_eq!((count, realm.as_str()), (2, "main"));
    assert_eq!(
        occ_taken_at,
        taken_at.naive_utc(),
        "the children carry the pass's clock, not now()"
    );

    let (w_count, live): (i32, Option<i32>) =
        sqlx::query_as("SELECT count, live_users FROM presence.world_membership WHERE snapshot_id = $1 AND world_name = 'empty.eth'")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!((w_count, live), (0, Some(3)));

    let snap: (i32, i32, i32, i32, i32, Option<i32>) = sqlx::query_as(
        "SELECT peers_count, scenes_polled, scene_users_total, worlds_polled, active_worlds, worlds_live_total \
         FROM presence.snapshots WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(snap, (3, 3, 2, 2, 1, Some(4)));

    // Still the pre-commit memo until the collector refreshes it.
    assert_eq!(
        state
            .queries
            .current_bundle()
            .await
            .unwrap()
            .current
            .as_ref()
            .map(|c| c.snapshot_id),
        before
    );
    state.queries.refresh_current().await.unwrap();
    let bundle = state.queries.current_bundle().await.unwrap();
    assert_eq!(bundle.current.as_ref().map(|c| c.snapshot_id), Some(id));
    assert_eq!(bundle.scenes.len(), 2);
    assert_eq!(bundle.scenes[0].pointer, "1,-2");
    assert_eq!(bundle.worlds.len(), 2);
    assert_eq!(bundle.worlds[0].world_name, "alice.eth");

    delete_snapshots(pool, &[id]).await;
}

#[tokio::test]
async fn snapshot_batches_preserve_duplicates_nulls_and_atomicity() {
    let _lock = DB_LOCK.lock().await;
    let tag = Uuid::new_v4().simple().to_string();
    let mut peers: Vec<Peer> = (0..513)
        .map(|i| Peer {
            address: format!("peer-{tag}-{i}"),
            parcel_x: None,
            parcel_y: None,
            position_x: None,
            position_y: None,
            position_z: None,
            last_ping: None,
        })
        .collect();
    peers.push(peer(&format!("peer-{tag}-0"), Some((99, 99))));
    let roster = vec![
        "0xAbC".to_string(),
        "0xabc".to_string(),
        "0xDef".to_string(),
    ];
    let sources = FakeSources {
        peers,
        islands: vec![Island {
            island_id: "island-1".into(),
            peer_count: 1,
            max_peers: None,
            center_x: None,
            center_y: None,
            center_z: None,
            radius: None,
        }],
        hot_scenes: vec![hot_scene("scene-1", Some("tour"), Some((0, 0)), Some(2), 1)],
        scenes: HashMap::from([("0,0".to_string(), Some(roster.clone()))]),
        worlds: HashMap::from([("test.eth".to_string(), Some(roster))]),
        live: Some(LiveData {
            total_users: 2,
            per_world: vec![("test.eth".into(), 2)],
        }),
    };
    let Some(state) = state(sources, true).await else {
        return;
    };
    let pool = &state.pool;

    let fn_name = format!("presence_count_insert_{tag}");
    let calls = format!("presence_insert_calls_{tag}");
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE {calls} (table_name text); \
         CREATE FUNCTION {fn_name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN INSERT INTO {calls} VALUES (TG_TABLE_NAME); RETURN NULL; END $$;"
    )))
    .execute(pool)
    .await
    .unwrap();
    let tables = [
        "snapshots",
        "peer_snapshots",
        "island_snapshots",
        "hot_scene_snapshots",
        "scene_occupancy",
        "world_membership",
    ];
    for table in tables {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE TRIGGER count_insert_{tag} AFTER INSERT ON presence.{table} FOR EACH STATEMENT EXECUTE FUNCTION {fn_name}()"
        )))
        .execute(pool)
        .await
        .unwrap();
    }

    let summary = collector::run_pass(&state, now()).await.unwrap();
    assert_eq!(
        (
            summary.peers,
            summary.islands,
            summary.hot_scenes,
            summary.scene_users,
            summary.world_users
        ),
        (514, 1, 1, 2, 2)
    );
    let id = summary.snapshot_id;
    let (peer_rows, nulls): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE parcel_x IS NULL AND last_ping IS NULL) FROM presence.peer_snapshots WHERE snapshot_id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        (peer_rows, nulls),
        (513, 513),
        "the duplicate address keeps the first (null) row"
    );
    let (addresses, count, live_users): (Value, i32, i32) = sqlx::query_as(
        "SELECT addresses, count, live_users FROM presence.world_membership WHERE snapshot_id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        (addresses, count, live_users),
        (json!(["0xabc", "0xdef"]), 2, 2)
    );
    let statements: i64 =
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {calls}")))
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(statements, 6, "one parent, five unnest child tables");
    assert_eq!(
        state
            .queries
            .current_bundle()
            .await
            .unwrap()
            .current
            .as_ref()
            .map(|c| c.snapshot_id),
        Some(id),
        "the pass refreshed the memo"
    );

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "ALTER TABLE presence.hot_scene_snapshots ADD CONSTRAINT fail_next_{tag} CHECK (snapshot_id = {id}) NOT VALID"
    )))
    .execute(pool)
    .await
    .unwrap();
    let err = collector::run_pass(&state, now())
        .await
        .expect_err("the child insert must fail the pass");
    assert!(format!("{err:#}").contains("insert hot scenes"), "{err:#}");
    let later: i64 = count_where(
        pool,
        "SELECT count(*) FROM presence.snapshots WHERE id > $1",
        id,
    )
    .await;
    assert_eq!(later, 0, "failure must roll back the entire snapshot");

    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "ALTER TABLE presence.hot_scene_snapshots DROP CONSTRAINT fail_next_{tag}; \
         DROP FUNCTION {fn_name}() CASCADE; DROP TABLE {calls};"
    )))
    .execute(pool)
    .await
    .unwrap();
    delete_snapshots(pool, &[id]).await;
}

#[tokio::test]
async fn routes_answer_the_envelope_while_the_archive_is_empty_and_the_archive_after_a_pass() {
    let _lock = DB_LOCK.lock().await;
    let tag = Uuid::new_v4().simple().to_string();
    let pointer = format!("{},{}", 1000 + (tag.as_bytes()[0] as i32), -1000);
    let world = format!("{tag}.dcl.eth");
    let sources = FakeSources {
        peers: vec![peer("0xaa", Some((1, -2)))],
        hot_scenes: vec![
            hot_scene("s1", Some("Plaza"), Some((1, -2)), Some(1), 1),
            hot_scene("s2", Some("Fair"), None, None, 0),
        ],
        scenes: HashMap::from([("1,-2".to_string(), Some(vec!["0xAA".to_string()]))]),
        worlds: HashMap::from([(
            world.clone(),
            Some(vec!["0xcc".to_string(), "0xdd".to_string()]),
        )]),
        live: Some(LiveData {
            total_users: 2,
            per_world: vec![(world.clone(), 2), ("quiet.eth".into(), 0)],
        }),
        ..FakeSources::default()
    };
    let Some(state) = state(sources, true).await else {
        return;
    };
    let pool = state.pool.clone();
    let app = Router::new().nest("/presence", router(state.clone()));

    // This module is the only writer of presence.*: an empty archive is a whole-table fact.
    let parked: Vec<(i64, chrono::NaiveDateTime)> =
        sqlx::query_as("SELECT id, taken_at FROM presence.snapshots")
            .fetch_all(&pool)
            .await
            .unwrap();
    sqlx::query("DELETE FROM presence.snapshots")
        .execute(&pool)
        .await
        .unwrap();
    state.queries.refresh_current().await.unwrap();
    for path in [
        "/presence/current",
        "/presence/current/scenes",
        "/presence/current/worlds",
        "/presence/scenes/history?pointer=1,-2",
        "/presence/worlds/history?world=x.eth",
    ] {
        let (status, retry_after, body) = get(&app, path).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(retry_after.as_deref(), Some("60"), "{path}");
        assert_eq!(body["error"], "Service Unavailable");
        assert_eq!(body["source"], SOURCE_NAME);
        assert_eq!(body["configure"], SOURCE_CONFIGURE);
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("DATA_WRITES_ENABLED=true"),
            "{body}"
        );
    }

    let first_at = now() - Duration::minutes(5);
    let mut first = sample(first_at);
    first.occupancy = vec![(pointer.clone(), Some("Old".into()), vec!["0x11".into()])];
    first.world_rows = vec![(world.clone(), vec!["0x11".into()], 1)];
    let first_id = store_snapshot(&pool, &first).await.unwrap();
    let pass_at = now();
    let summary = collector::run_pass(&state, pass_at).await.unwrap();
    assert_eq!(
        (
            summary.scenes_polled,
            summary.scene_users,
            summary.worlds_polled,
            summary.world_users
        ),
        (1, 1, 1, 2)
    );

    let (status, _, body) = get(&app, "/presence/current").await;
    assert_eq!(status, StatusCode::OK);
    let current = &body["current"];
    assert_eq!(current["snapshot_id"], summary.snapshot_id);
    assert_eq!(
        current["taken_at"],
        pass_at
            .naive_utc()
            .and_utc()
            .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
    );
    assert_eq!(
        (
            current["peers_count"].as_i64(),
            current["worlds_live_total"].as_i64()
        ),
        (Some(1), Some(2))
    );

    let (status, _, body) = get(&app, "/presence/current/scenes").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["scenes"].as_array().unwrap().len(),
        1,
        "the scene without a base parcel is never polled"
    );
    assert_eq!(body["scenes"][0]["pointer"], "1,-2");
    assert_eq!(body["scenes"][0]["count"], 1);
    assert_eq!(body["scenes"][0]["realm"], "main");

    let (status, _, body) = get(&app, "/presence/current/worlds").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["worlds"],
        json!([{ "taken_at": current["taken_at"], "world_name": world, "count": 2, "live_users": 2 }])
    );

    let (status, _, body) = get(&app, &format!("/presence/worlds/history?world={world}")).await;
    assert_eq!(status, StatusCode::OK);
    let history = body["history"].as_array().unwrap();
    assert_eq!(history.len(), 2, "newest first, across snapshots");
    assert_eq!(
        (history[0]["count"].as_i64(), history[1]["count"].as_i64()),
        (Some(2), Some(1))
    );
    let (_, _, body) = get(
        &app,
        &format!("/presence/worlds/history?world={world}&limit=1"),
    )
    .await;
    assert_eq!(body["history"].as_array().unwrap().len(), 1);
    let (_, _, body) = get(
        &app,
        &format!("/presence/worlds/history?world={world}&limit=0"),
    )
    .await;
    assert_eq!(
        body["history"].as_array().unwrap().len(),
        2,
        "a non-positive limit is the default"
    );

    let (status, _, body) = get(&app, &format!("/presence/scenes/history?pointer={pointer}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["pointer"], pointer);
    assert_eq!(
        body["history"],
        json!([{ "taken_at": first_at.naive_utc().and_utc().to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true), "pointer": pointer, "scene_name": "Old", "realm": "main", "count": 1 }])
    );
    let (status, _, body) = get(&app, "/presence/scenes/history?pointer=999,999").await;
    assert_eq!(
        (status, body["history"].as_array().unwrap().len()),
        (StatusCode::OK, 0),
        "an unknown pointer is an empty history once the archive has a pass"
    );
    let (status, _, _) = get(&app, "/presence/scenes/history").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "pointer is required");

    delete_snapshots(&pool, &[first_id, summary.snapshot_id]).await;
    if !parked.is_empty() {
        // Whatever another presence test left behind is gone; the lock made that impossible, but say so.
        panic!(
            "presence.snapshots held {} rows from outside this test: {:?}",
            parked.len(),
            parked
        );
    }
}

#[tokio::test]
async fn writes_disabled_attaches_no_collector_and_a_pass_refuses() {
    let state = state_without_db(false);
    let jobs = Jobs::new();
    state.spawn_jobs(&jobs);
    assert!(
        jobs.report().is_empty(),
        "no job without DATA_WRITES_ENABLED"
    );
    let err = collector::run_pass(&state, now())
        .await
        .expect_err("a pass on a read-only node must refuse before reading anything");
    assert!(
        err.to_string().contains("DATA_WRITES_ENABLED=false"),
        "{err}"
    );

    let state = state_without_db(true);
    state.spawn_jobs(&jobs);
    let report = jobs.report();
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].name, "presence-collector");
    jobs.shutdown(std::time::Duration::from_secs(1)).await;
}

/// `PRESENCE_COLLECTOR_ENABLED`, when set, decides the collector over
/// `DATA_WRITES_ENABLED`, and the refusals name it: the pass's error and
/// the empty archive's envelope.
#[tokio::test]
async fn the_collector_override_is_the_switch_a_refusal_names() {
    let off = Settings::from_switches(Some(false), true);
    assert_eq!((off.writes_enabled, off.switch), (false, COLLECTOR_SWITCH));
    let on = Settings::from_switches(Some(true), false);
    assert_eq!((on.writes_enabled, on.switch), (true, COLLECTOR_SWITCH));
    let unset = Settings::from_switches(None, false);
    assert_eq!(
        (unset.writes_enabled, unset.switch),
        (false, SOURCE_CONFIGURE)
    );

    let err = collector::run_pass(&state_without_db_with(off), now())
        .await
        .expect_err("a disabled collector refuses");
    assert_eq!(
        err.to_string(),
        "the presence collector records nothing while PRESENCE_COLLECTOR_ENABLED=false"
    );
    let res = unavailable(COLLECTOR_SWITCH);
    let body: Value =
        serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 4096).await.unwrap())
            .unwrap();
    assert_eq!(body["configure"], COLLECTOR_SWITCH);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("PRESENCE_COLLECTOR_ENABLED=true"),
        "{body}"
    );
}

#[tokio::test]
async fn the_retention_sweep_is_bounded_and_keeps_recent_snapshots() {
    let _lock = DB_LOCK.lock().await;
    let Some(state) = state(FakeSources::default(), true).await else {
        return;
    };
    let pool = &state.pool;
    let at = now();
    let mut ids = Vec::new();
    for days_ago in [30, 20, 10, 8, 1] {
        let mut data = sample(at - Duration::days(days_ago));
        data.peers.clear();
        ids.push(store_snapshot(pool, &data).await.unwrap());
    }
    let cutoff = (at - Duration::days(7)).naive_utc();
    assert_eq!(
        collector::sweep_retention(pool, cutoff, 3).await.unwrap(),
        3,
        "oldest first, at most the limit"
    );
    let left: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM presence.snapshots WHERE id = ANY($1) ORDER BY id")
            .bind(&ids)
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(left, ids[3..].to_vec());
    assert_eq!(
        collector::sweep_retention(pool, cutoff, 3).await.unwrap(),
        1
    );
    assert_eq!(
        collector::sweep_retention(pool, cutoff, 3).await.unwrap(),
        0
    );
    let children: i64 = count_where(
        pool,
        "SELECT count(*) FROM presence.scene_occupancy WHERE snapshot_id = $1",
        ids[0],
    )
    .await;
    assert_eq!(children, 0, "the children went with the snapshot");
    let recent: i64 = count_where(
        pool,
        "SELECT count(*) FROM presence.world_membership WHERE snapshot_id = $1",
        ids[4],
    )
    .await;
    assert_eq!(recent, 2);
    delete_snapshots(pool, &ids).await;
}

#[tokio::test]
async fn daily_stats_rolls_a_day_up_and_rebuilds_on_rerun() {
    let _lock = DB_LOCK.lock().await;
    let Some(state) = state(FakeSources::default(), true).await else {
        return;
    };
    let pool = &state.pool;
    let day = NaiveDate::from_ymd_opt(2001, 2, 3).unwrap();
    let at = |h: u32| day.and_hms_opt(h, 0, 0).unwrap().and_utc();
    let mut a = sample(at(1));
    a.peers.truncate(1);
    let b = sample(at(23));
    let ids = vec![
        store_snapshot(pool, &a).await.unwrap(),
        store_snapshot(pool, &b).await.unwrap(),
    ];

    collector::aggregate_day(pool, day).await.unwrap();
    let row: (i32, i32, f64, i32, i32, i32) = sqlx::query_as(
        "SELECT snapshots_taken, peak_peers, avg_peers, peak_hot_scene_users, peak_scene_users, peak_world_users \
         FROM presence.daily_stats WHERE date = $1",
    )
    .bind(day)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(row, (2, 3, 2.0, 2, 2, 1));

    delete_snapshots(pool, &ids[1..]).await;
    collector::aggregate_day(pool, day).await.unwrap();
    let (taken, peak): (i32, i32) = sqlx::query_as(
        "SELECT snapshots_taken, peak_peers FROM presence.daily_stats WHERE date = $1",
    )
    .bind(day)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        (taken, peak),
        (1, 1),
        "derived: re-aggregating rebuilds the row"
    );

    delete_snapshots(pool, &ids).await;
    sqlx::query("DELETE FROM presence.daily_stats WHERE date = $1")
        .bind(day)
        .execute(pool)
        .await
        .unwrap();
}

mod module_sources {
    use crate::comms::{
        gatekeeper::bus::InProcessBus,
        protocol::bus::statistics::{
            Change, Frames, Peer, SNAPSHOTS_SUBJECT, Snapshot, Update, frames,
        },
        stats,
    };
    use async_trait::async_trait;
    use std::sync::Arc;
    struct UpdatingContent {
        bus: Arc<InProcessBus>,
    }
    #[async_trait]
    impl stats::content::Source for UpdatingContent {
        async fn fetch(
            &self,
            pointers: &[String],
        ) -> Result<Vec<crate::content::OrderedJson>, stats::content::Error> {
            assert_eq!(pointers, &["0,0"]);
            let update = Update::Change {
                revision: 2,
                change: Change::Remove("MixedCase".into()),
            };
            for frame in Frames::new(&update, uuid::Uuid::nil(), 2, 65536).unwrap() {
                self.bus.inject(SNAPSHOTS_SUBJECT, &frame);
            }
            Ok(vec![
                serde_json::from_str(
                    r#"{"id":"scene", "metadata":{"scene":{"base":"0,0","parcels":["0,0"]}}}"#,
                )
                .unwrap(),
            ])
        }
    }

    #[tokio::test]
    async fn validated_historical_sync_order_reaches_hot_scenes_http() {
        use crate::content::sync_deployer::{LiveSyncDeployer, SyncValidation};
        use alloy::signers::{Signer as _, local::PrivateKeySigner};
        let pool = crate::db::scratch::pool(3)
            .await
            .expect("test database required");
        let temp = std::env::temp_dir().join(format!("ordered-ingestion-{}", uuid::Uuid::new_v4()));
        let storage = Arc::new(crate::storage::Storage::from_config(&temp).await.unwrap());
        let code = bytes::Bytes::from_static(b"export function onUpdate() {}");
        let hash = dcl_auth::hash_bytes_v1(&code);
        storage.store(&hash, code).await.unwrap();
        // Historical sync legitimately skips the modern primitive-type metadata schema.
        // Access is explicitly controlled; all other content validation runs.
        let metadata = r#"{"main":"bin/index.js","scene":{"base":"23,24","parcels":["23,24"]},"display":{"title":{"z":9007199254740993,"a":2},"description":"archive text"},"contact":{"name":"creator"},"source":{"origin":"builder","projectId":"project"}}"#;
        let raw = format!(
            r#"{{"version":"v3","type":"scene","pointers":["23,24"],"timestamp":1650000000000,"content":[{{"file":"bin/index.js","hash":"{hash}"}}],"metadata":{metadata}}}"#
        );
        let id = dcl_auth::hash_bytes_v1(raw.as_bytes());
        storage
            .store(&id, bytes::Bytes::copy_from_slice(raw.as_bytes()))
            .await
            .unwrap();
        let key = PrivateKeySigner::random();
        let chain = serde_json::from_value(serde_json::json!([
            {"type":"SIGNER","payload":format!("{:#x}",key.address()),"signature":""},
            {"type":"ECDSA_SIGNED_ENTITY","payload":id,"signature":key.sign_message(id.as_bytes()).await.unwrap().to_string()}
        ])).unwrap();
        let sources = crate::content::validator::Sources::new(
            pool.clone(),
            storage.clone(),
            crate::http::Outbound::for_tests(),
            None,
            crate::content::validator::Settings {
                ignore_blockchain_access: true,
                eth_network: "mainnet".into(),
                ..Default::default()
            },
        );
        let deployer =
            LiveSyncDeployer::new(pool.clone(), 1, SyncValidation::Content(Arc::new(sources)));
        deployer
            .deploy_entity(
                raw.as_bytes(),
                &id,
                &chain,
                crate::content::sync::DeploymentContext::Synced,
                None,
                None,
            )
            .await
            .unwrap();
        deployer.flush().await.unwrap();
        assert_eq!(
            storage.retrieve(&id).await.unwrap().unwrap().as_ref(),
            raw.as_bytes()
        );
        let stored: String = sqlx::query_scalar(
            "SELECT entity_metadata::text FROM content.deployments WHERE entity_id=$1",
        )
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stored, format!(r#"{{"v":{metadata}}}"#));
        eprintln!("historical_sync_sql={stored}");
        let reader = Arc::new(stats::Reader::default());
        let bus = InProcessBus::new();
        let _sub = reader.subscribe(&bus);
        let source = Arc::new(stats::content::Content {
            pool: pool.clone(),
            denylist: Arc::new(crate::content::Denylist::default()),
        });
        let snapshot = Snapshot {
            peers: vec![Peer {
                address: "historical".into(),
                position: [368., 0., 384.],
                time: 1,
            }],
            ..Default::default()
        };
        for frame in frames(&snapshot, uuid::Uuid::nil(), 1, 65536).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
        let archive = super::super::sources::statistics(&reader, source.as_ref(), "")
            .await
            .unwrap();
        assert_eq!(archive.hot_scenes.len(), 1);
        assert_eq!(archive.hot_scenes[0].scene_id, id);
        assert_eq!(archive.hot_scenes[0].name, None);
        assert_eq!(
            archive.hot_scenes[0].description.as_deref(),
            Some("archive text")
        );
        assert_eq!(archive.hot_scenes[0].creator.as_deref(), Some("creator"));
        struct Hooks;
        #[async_trait]
        impl stats::http::Hooks for Hooks {
            async fn probe(&self, _: bool) -> stats::http::Lifecycle {
                Default::default()
            }
            async fn metrics(&self) -> Result<String, stats::content::Error> {
                Ok(String::new())
            }
            async fn diagnostics(&self) -> Result<String, stats::content::Error> {
                Ok(String::new())
            }
            fn observe(&self, _: stats::http::Observation) {}
        }
        let app = stats::http::standalone(stats::http::State::new(
            reader,
            source,
            Arc::new(Hooks),
            Default::default(),
            stats::http::test_support::matcher(),
        ));
        async fn request(
            app: axum::Router,
            method: &str,
            path: &str,
        ) -> (axum::http::StatusCode, axum::http::HeaderMap, String) {
            use tower::ServiceExt;
            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method(method)
                        .uri(path)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            (status, headers, String::from_utf8(body.to_vec()).unwrap())
        }
        let (status, headers, body) = request(app.clone(), "GET", "/hot-scenes").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            body,
            format!(
                r#"[{{"id":"{id}","name":{{"z":9007199254740992,"a":2}},"baseCoords":[23,24],"usersTotalCount":1,"parcels":[[23,24]],"creator":"creator","projectId":"project","description":"archive text"}}]"#
            )
        );
        assert_eq!(headers["content-length"], body.len().to_string());
        eprintln!("historical_sync_hot_scenes={body}");
        assert!(request(app, "HEAD", "/hot-scenes").await.2.is_empty());
        sqlx::query("DELETE FROM content.active_pointers WHERE entity_id=$1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.content_files WHERE deployment IN (SELECT id FROM content.deployments WHERE entity_id=$1)").bind(&id).execute(&pool).await.unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id=$1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        drop(storage);
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[tokio::test]
    async fn selected_sql_archive_observes_shared_content_policy() {
        let pool = crate::db::scratch::pool(2)
            .await
            .expect("test database required");
        let tag = uuid::Uuid::new_v4();
        let coordinate = 1_600_000 + (tag.as_u128() % 100_000) as i32;
        let ids = [format!("allowed-{tag}"), format!("denied-{tag}")];
        // Synthetic persisted rows test the read policy, not deployment admission.
        for (i, id) in ids.iter().enumerate() {
            let pointer = format!("{},0", coordinate + i as i32);
            let metadata = serde_json::json!({"v":{"scene":{"base":pointer,"parcels":[pointer]},"display":{"title":"allowed title","description":"text"},"contact":{"name":"creator"}}});
            sqlx::query("INSERT INTO content.deployments (deployer_address,version,entity_type,entity_id,entity_metadata,entity_timestamp,entity_pointers,local_timestamp,auth_chain) VALUES ('0x0000000000000000000000000000000000000001','v3','scene',$1,$2,to_timestamp(1),$3,to_timestamp(1),'[]'::json)").bind(id).bind(metadata).bind(vec![pointer.clone()]).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO content.active_pointers (pointer,entity_id,entity_type) VALUES ($1,$2,'scene')").bind(pointer).bind(id).execute(&pool).await.unwrap();
        }
        let denylist = Arc::new(crate::content::Denylist::default());
        denylist.set_operator([ids[1].clone()]);
        let source = stats::content::Content {
            pool,
            denylist: denylist.clone(),
        };
        let reader = Arc::new(stats::Reader::default());
        let bus = InProcessBus::new();
        let _sub = reader.subscribe(&bus);
        let snapshot = Snapshot {
            peers: (0..2)
                .map(|i| Peer {
                    address: format!("peer-{i}"),
                    position: [f64::from(coordinate + i) * 16., 0., 0.],
                    time: 1,
                })
                .collect(),
            ..Default::default()
        };
        for frame in frames(&snapshot, uuid::Uuid::nil(), 1, 65536).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
        let result = super::super::sources::statistics(&reader, &source, "")
            .await
            .unwrap();
        assert_eq!(
            result.peers.len(),
            2,
            "content policy leaves the retained population intact"
        );
        assert_eq!(result.hot_scenes.len(), 1);
        let scene = &result.hot_scenes[0];
        assert_eq!(scene.scene_id, ids[0]);
        assert_eq!(scene.name.as_deref(), Some("allowed title"));
        assert_eq!(scene.description.as_deref(), Some("text"));
        assert_eq!(scene.creator.as_deref(), Some("creator"));
        assert_eq!(scene.users_count, Some(1));
        denylist.set_operator(ids);
        let next = super::super::sources::statistics(&reader, &source, "")
            .await
            .unwrap();
        assert!(next.hot_scenes.is_empty());
        assert_eq!(next.peers.len(), 2);
        source.pool.close().await;
        assert!(
            super::super::sources::statistics(&reader, &source, "")
                .await
                .is_err(),
            "SQL failure remains an archive failure even when every ID is denied"
        );
        for frame in frames(&Snapshot::default(), uuid::Uuid::nil(), 2, 65536).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
        let empty = super::super::sources::statistics(&reader, &source, "")
            .await
            .unwrap();
        assert!(
            empty.hot_scenes.is_empty(),
            "empty population skips the closed SQL source"
        );
        assert!(empty.peers.is_empty());
    }

    #[tokio::test]
    async fn archive_reads_selected_sql_metadata_and_omits_non_string_fields() {
        let pool = crate::db::scratch::pool(2)
            .await
            .expect("test database required");
        let id = format!("archive-{}", uuid::Uuid::new_v4());
        let coordinate = 1_500_000 + (uuid::Uuid::new_v4().as_u128() % 100_000) as i32;
        let pointer = format!("{coordinate},0");
        let metadata = format!(
            r#"{{"v":{{"scene":{{"base":"{pointer}","parcels":["{pointer}","{pointer}"]}},"display":{{"title":{{"z":1,"a":2}},"description":{{"z":true,"a":null}}}},"contact":{{"name":["legacy"]}}}}}}"#
        );
        sqlx::query("INSERT INTO content.deployments (deployer_address,version,entity_type,entity_id,entity_metadata,entity_timestamp,entity_pointers,local_timestamp,auth_chain) VALUES ('0x0000000000000000000000000000000000000001','v3','scene',$1,$2::json,to_timestamp(1),$3,to_timestamp(1),'[]'::json)").bind(&id).bind(metadata).bind(vec![pointer.clone()]).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO content.active_pointers (pointer,entity_id,entity_type) VALUES ($1,$2,'scene')").bind(&pointer).bind(&id).execute(&pool).await.unwrap();
        let reader = Arc::new(stats::Reader::default());
        let bus = InProcessBus::new();
        let _sub = reader.subscribe(&bus);
        let snapshot = Snapshot {
            peers: vec![Peer {
                address: "archive".into(),
                position: [f64::from(coordinate) * 16., 0., 0.],
                time: 1,
            }],
            ..Default::default()
        };
        for frame in frames(&snapshot, uuid::Uuid::nil(), 1, 65536).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
        let source = stats::content::Content {
            pool,
            denylist: Arc::new(crate::content::Denylist::default()),
        };
        let result = super::super::sources::statistics(&reader, &source, "")
            .await
            .unwrap();
        let scene = &result.hot_scenes[0];
        assert_eq!(scene.scene_id, id);
        assert_eq!(scene.users_count, Some(2));
        assert_eq!(scene.parcel_count, 2);
        assert_eq!(scene.base_x, Some(coordinate));
        assert_eq!(scene.name, None);
        assert_eq!(scene.creator, None);
        assert_eq!(scene.description, None);
        use stats::content::Source;
        let docs = source.fetch(&[pointer.clone()]).await.unwrap();
        let http = stats::content::hot_scenes(&docs, &[(pointer, 1)], "").unwrap();
        assert!(http.contains(r#""name":{"z":1,"a":2}"#));
        assert!(http.contains(r#""usersTotalCount":2"#));
        assert!(http.contains(&format!(r#""id":"{id}""#)));
    }

    #[tokio::test]
    async fn archive_rejects_count_overflow_instead_of_saturating() {
        use prost::Message;
        let bus = Arc::new(InProcessBus::new());
        let reader = Arc::new(stats::Reader::default());
        let _subscription = reader.subscribe(bus.as_ref());
        let snapshot = Snapshot {
            islands: crate::comms::protocol::archipelago::archipelago::IslandStatusMessage {
                data: vec![
                    crate::comms::protocol::archipelago::archipelago::IslandData {
                        id: "oversized-capacity".into(),
                        max_peers: u32::MAX,
                        center: Some(Default::default()),
                        ..Default::default()
                    },
                ],
            }
            .encode_to_vec(),
            ..Default::default()
        };
        for frame in frames(&snapshot, uuid::Uuid::nil(), 1, 65536).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
        assert!(
            super::super::sources::statistics(&reader, &UpdatingContent { bus }, "")
                .await
                .is_err()
        );
    }

    fn numeric_snapshot() -> Snapshot {
        use crate::comms::protocol::archipelago::{
            Position,
            archipelago::{
                IslandData, IslandStatusMessage, ServiceDiscoveryMessage, ServiceStatus,
            },
        };
        use prost::Message;
        Snapshot {
            peers: [
                ("negative", [-0.5, 0., 0.]),
                ("large", [f64::from(2_f32.powi(35)), 0., 0.]),
                ("zero", [-0., 0., -0.]),
                ("extreme", [f64::from(f32::MAX), 0., -f64::from(f32::MAX)]),
                ("nonfinite", [f64::NAN, f64::INFINITY, f64::NEG_INFINITY]),
            ]
            .into_iter()
            .map(|(address, position)| Peer {
                address: address.into(),
                position,
                time: 1_700_000_000_123,
            })
            .collect(),
            islands: IslandStatusMessage {
                data: vec![IslandData {
                    id: "supported".into(),
                    peers: ["negative", "unknown", "large", "negative", "nonfinite"]
                        .map(str::to_owned)
                        .to_vec(),
                    max_peers: 0,
                    center: Some(Position {
                        x: 0.1,
                        y: 2.,
                        z: 3.,
                    }),
                    radius: 1.0000000001,
                }],
            }
            .encode_to_vec(),
            discovery: ServiceDiscoveryMessage {
                status: Some(ServiceStatus {
                    current_time: chrono::Utc::now().timestamp_millis() as u64,
                    user_count: u32::MAX,
                    ..Default::default()
                }),
                ..Default::default()
            }
            .encode_to_vec(),
        }
    }

    fn publish_snapshot(bus: &InProcessBus, snapshot: &Snapshot, revision: u64) {
        for frame in frames(snapshot, uuid::Uuid::nil(), revision, 65536).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
    }

    #[tokio::test]
    async fn numeric_statistics_reach_about_and_realm_routes_without_signed_narrowing() {
        use crate::content::about_comms::{AboutCommsState, CommsCore, Settings};
        let reader = Arc::new(stats::Reader::default());
        let bus = InProcessBus::new();
        let _subscription = reader.subscribe(&bus);
        let snapshot = numeric_snapshot();
        publish_snapshot(&bus, &snapshot, 1);
        assert_eq!(
            reader.realm_view(),
            (u64::from(u32::MAX), vec![((-1, 0), 1), ((0, 0), 1)])
        );

        let files = std::env::temp_dir().join(format!("numeric-about-{}", uuid::Uuid::new_v4()));
        let storage = Arc::new(crate::storage::Storage::from_config(&files).await.unwrap());
        let mut settings = crate::content::Settings::for_tests();
        settings.about_resource_check = false;
        settings.content_public_url = "https://content.example/content".into();
        settings.realm_name = Some("numeric".into());
        let content = Arc::new(crate::content::ContentState::new(
            super::state_without_db(true).pool.clone(),
            settings,
            storage,
        ));
        content.sync.set_state(crate::content::SyncState::Syncing);
        let max_users = u64::from(u32::MAX) + 1;
        let about = Arc::new(AboutCommsState::new(Settings {
            protocol: "v3".into(),
            fixed_adapter: "ws-room:wss://comms.example/ws".into(),
            version: None,
            commit_hash: None,
            v4: None,
            max_users: Some(max_users),
            offline_when_unreachable: false,
        }));
        assert!(about.attach_core(reader.clone()));
        assert!(content.attach_about_comms(about));
        let app = crate::content::build_router(content.clone());
        for path in ["/about", "/lambdas/about"] {
            let (status, _, body) = super::get(&app, path).await;
            assert_eq!(status, axum::http::StatusCode::OK);
            assert_eq!(body["comms"]["usersCount"], u32::MAX);
            assert_eq!(body["comms"]["healthy"], true);
            assert_eq!(body["acceptingUsers"], true);
        }
        let (status, _, body) = super::get(&app, "/lambdas/explore/realms").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            body,
            serde_json::json!([{
                "serverName":"numeric", "url":"https://content.example",
                "usersCount":u32::MAX, "maxUsers":max_users,
                "userParcels":[[-1,0],[0,0]],
            }])
        );
        // Advertised population and representable parcel population are independent.
        assert_eq!(reader.snapshot().peers.len(), 5);
        drop(app);
        drop(content);
        std::fs::remove_dir_all(files).unwrap();
    }

    type ContentResult = Result<Vec<crate::content::OrderedJson>, stats::content::Error>;

    struct BlockedContent {
        entered: tokio::sync::Semaphore,
        release: tokio::sync::Semaphore,
        next: std::sync::Mutex<Option<ContentResult>>,
    }
    impl BlockedContent {
        fn new() -> Self {
            Self {
                entered: tokio::sync::Semaphore::new(0),
                release: tokio::sync::Semaphore::new(0),
                next: std::sync::Mutex::new(None),
            }
        }
        async fn wait_for_fetch(&self) {
            tokio::time::timeout(std::time::Duration::from_secs(5), self.entered.acquire())
                .await
                .expect("collector reaches content I/O")
                .unwrap()
                .forget();
        }
    }
    #[async_trait]
    impl stats::content::Source for BlockedContent {
        async fn fetch(
            &self,
            pointers: &[String],
        ) -> Result<Vec<crate::content::OrderedJson>, stats::content::Error> {
            assert_eq!(
                pointers,
                &[
                    "-1,0",
                    "2147483648,0",
                    "0,0",
                    "2.1267646664908054e+37,-2.1267646664908054e+37",
                    "NaN,-Infinity",
                ]
            );
            self.entered.add_permits(1);
            self.release.acquire().await.unwrap().forget();
            self.next
                .lock()
                .unwrap()
                .take()
                .expect("controlled content outcome")
        }
    }
    struct ReaderSources {
        reader: Arc<stats::Reader>,
        content: Arc<BlockedContent>,
    }
    #[async_trait]
    impl super::Sources for ReaderSources {
        async fn statistics(&self) -> anyhow::Result<super::super::sources::Statistics> {
            super::super::sources::statistics(
                &self.reader,
                self.content.as_ref(),
                "https://content.example/content/",
            )
            .await
        }
        async fn scene_participants(
            &self,
            realm: &str,
            pointer: &str,
        ) -> anyhow::Result<Option<Vec<String>>> {
            assert_eq!((realm, pointer), ("main", "-1,0"));
            Ok(Some(vec!["0xAA".into(), "0xaa".into()]))
        }
        async fn world_participants(&self, _: &str) -> anyhow::Result<Option<Vec<String>>> {
            panic!("no active worlds in this pass")
        }
        async fn worlds_live_data(&self) -> anyhow::Result<Option<super::LiveData>> {
            Ok(None)
        }
    }
    fn archive_document() -> crate::content::OrderedJson {
        serde_json::from_str(r#"{"id":"retained","metadata":{"scene":{"base":"-1,0","parcels":["-1,0","-1,0"]},"display":{"title":"Retained"}}}"#).unwrap()
    }

    #[tokio::test]
    async fn archive_retains_one_statistics_snapshot_while_content_awaits() {
        use crate::comms::protocol::archipelago::archipelago::IslandStatusMessage;
        use prost::Message;
        let _lock = super::DB_LOCK.lock().await;
        let pool = crate::db::scratch::pool(4)
            .await
            .expect("test database required");
        let reader = Arc::new(stats::Reader::default());
        let bus = InProcessBus::new();
        let _subscription = reader.subscribe(&bus);
        let snapshot = numeric_snapshot();
        publish_snapshot(&bus, &snapshot, 1);
        let content = Arc::new(BlockedContent::new());
        *content.next.lock().unwrap() = Some(Ok(vec![archive_document()]));
        let state = Arc::new(super::PresenceState::new(
            pool.clone(),
            super::Settings::new(true),
            Arc::new(ReaderSources {
                reader: reader.clone(),
                content: content.clone(),
            }),
        ));
        let app = axum::Router::new().nest("/presence", super::router(state.clone()));
        let paths = [
            "/presence/current",
            "/presence/current/scenes",
            "/presence/current/worlds",
            "/presence/scenes/history?pointer=-1,0",
        ];
        let mut previous = Vec::new();
        for path in paths {
            previous.push(super::get(&app, path).await);
        }
        let pass_at = super::now();
        *state.last_aggregated.lock() = Some((pass_at - chrono::Duration::days(1)).date_naive());
        let before: i64 = sqlx::query_scalar("SELECT count(*) FROM presence.snapshots")
            .fetch_one(&pool)
            .await
            .unwrap();
        let running = state.clone();
        let pass = tokio::spawn(async move { super::collector::run_pass(&running, pass_at).await });
        content.wait_for_fetch().await;
        publish_snapshot(&bus, &Snapshot::default(), 2);
        assert!(reader.snapshot().peers.is_empty());
        let during: i64 = sqlx::query_scalar("SELECT count(*) FROM presence.snapshots")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            during, before,
            "blocked source I/O must not persist a partial pass"
        );
        for (path, expected) in paths.into_iter().zip(&previous) {
            assert_eq!(
                &super::get(&app, path).await,
                expected,
                "blocked source I/O must preserve {path}"
            );
        }
        content.release.add_permits(1);
        let summary = tokio::time::timeout(std::time::Duration::from_secs(5), pass)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                summary.peers,
                summary.islands,
                summary.hot_scenes,
                summary.scene_users
            ),
            (5, 1, 1, 1)
        );
        let id = summary.snapshot_id;
        type PeerRow = (String, Vec<Option<i32>>, Vec<Option<f64>>, Option<i64>);
        let peers: Vec<PeerRow> = sqlx::query_as(
            "SELECT address, ARRAY[parcel_x,parcel_y], ARRAY[position_x,position_y,position_z], last_ping FROM presence.peer_snapshots WHERE snapshot_id=$1 ORDER BY address"
        ).bind(id).fetch_all(&pool).await.unwrap();
        let t = Some(1_700_000_000_123);
        assert_eq!(
            peers,
            vec![
                (
                    "extreme".into(),
                    vec![None, None],
                    vec![
                        Some(f64::from(f32::MAX)),
                        Some(0.),
                        Some(-f64::from(f32::MAX))
                    ],
                    t
                ),
                (
                    "large".into(),
                    vec![None, Some(0)],
                    vec![Some(34_359_738_368.), Some(0.), Some(0.)],
                    t
                ),
                (
                    "negative".into(),
                    vec![Some(-1), Some(0)],
                    vec![Some(-0.5), Some(0.), Some(0.)],
                    t
                ),
                (
                    "nonfinite".into(),
                    vec![None, None],
                    vec![None, None, None],
                    t
                ),
                (
                    "zero".into(),
                    vec![Some(0), Some(0)],
                    vec![Some(0.), Some(0.), Some(0.)],
                    t
                ),
            ]
        );
        type IslandRow = (String, i32, Option<i32>, Vec<Option<f64>>, Option<f64>);
        let island: IslandRow = sqlx::query_as(
            "SELECT island_id, peer_count, max_peers, ARRAY[center_x,center_y,center_z], radius FROM presence.island_snapshots WHERE snapshot_id=$1"
        ).bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(
            island,
            (
                "supported".into(),
                4,
                Some(0),
                vec![Some(f64::from(0.1_f32)), Some(2.), Some(3.)],
                Some(1.0000000001)
            )
        );
        let scene: (String, Option<i32>, Option<i32>, Option<i32>, i32) = sqlx::query_as(
            "SELECT scene_id, base_x, base_y, users_count, parcel_count FROM presence.hot_scene_snapshots WHERE snapshot_id=$1"
        ).bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(scene, ("retained".into(), Some(-1), Some(0), Some(2), 2));
        let mut committed = Vec::new();
        for path in paths {
            let (status, _, body) = super::get(&app, path).await;
            assert_eq!(status, axum::http::StatusCode::OK, "{path}");
            committed.push(body);
        }
        assert_eq!(committed[0]["current"]["snapshot_id"], id);
        assert_eq!(committed[0]["current"]["peers_count"], 5);
        assert_eq!(committed[0]["current"]["islands_count"], 1);
        assert_eq!(committed[0]["current"]["hot_scenes_count"], 1);
        assert_eq!(committed[1]["scenes"][0]["pointer"], "-1,0");
        assert_eq!(committed[1]["scenes"][0]["scene_name"], "Retained");
        assert_eq!(committed[1]["scenes"][0]["count"], 1);
        assert_eq!(committed[2]["worlds"], serde_json::json!([]));
        assert_eq!(committed[3]["history"].as_array().unwrap().len(), 1);
        assert_eq!(committed[3]["history"][0], committed[1]["scenes"][0]);

        for (index, (answer, expected_error)) in [
            (
                Err(stats::content::Error),
                "archive active content query failed",
            ),
            (
                Ok(vec![
                    serde_json::from_str(
                        r#"{"id":"bad","metadata":{"scene":{"base":true,"parcels":[]}}}"#,
                    )
                    .unwrap(),
                ]),
                "archive hot scene projection failed",
            ),
            (
                Ok(vec![archive_document()]),
                "synthetic maxPeers conversion",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let mut next = snapshot.clone();
            if index == 2 {
                // This exceeds the current producer's maxPeers=0; it tests checked storage only.
                let mut islands = IslandStatusMessage::decode(next.islands.as_slice()).unwrap();
                islands.data[0].max_peers = u32::MAX;
                next.islands = islands.encode_to_vec();
            }
            publish_snapshot(&bus, &next, 3 + index as u64);
            *content.next.lock().unwrap() = Some(answer);
            let running = state.clone();
            let pass = tokio::spawn(async move {
                super::collector::run_pass(
                    &running,
                    pass_at + chrono::Duration::seconds(index as i64 + 1),
                )
                .await
            });
            content.wait_for_fetch().await;
            content.release.add_permits(1);
            let error = tokio::time::timeout(std::time::Duration::from_secs(5), pass)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err();
            if index == 2 {
                assert!(error.is::<std::num::TryFromIntError>(), "{error:#}");
            } else {
                assert!(format!("{error:#}").contains(expected_error), "{error:#}");
            }
            assert_eq!(
                super::count_where(
                    &pool,
                    "SELECT count(*) FROM presence.snapshots WHERE id > $1",
                    id
                )
                .await,
                0
            );
            for (path, expected) in paths.into_iter().zip(&committed) {
                let (status, _, body) = super::get(&app, path).await;
                assert_eq!(status, axum::http::StatusCode::OK);
                assert_eq!(&body, expected, "failed pass must preserve {path}");
            }
        }
        super::delete_snapshots(&pool, &[id]).await;
    }
}

#[test]
fn history_limit_clamps_like_the_old_route() {
    assert_eq!(clamp_history_limit(None), 200);
    assert_eq!(clamp_history_limit(Some(0)), 200);
    assert_eq!(clamp_history_limit(Some(-5)), 200);
    assert_eq!(clamp_history_limit(Some(50)), 50);
    assert_eq!(clamp_history_limit(Some(9000)), 5000);
}

#[test]
fn rows_serialize_utc_timestamps_in_the_old_wire_form() {
    let row = WorldHeadcountRow {
        taken_at: chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .unwrap()
            .naive_utc(),
        world_name: "x.dcl.eth".into(),
        count: 2,
        live_users: None,
    };
    let v = serde_json::to_value(&row).unwrap();
    assert_eq!(v["taken_at"], "2023-11-14T22:13:20Z");
    assert!(v["live_users"].is_null());
}

#[tokio::test]
async fn the_envelope_has_the_data_plane_shape() {
    let ours = unavailable(SOURCE_CONFIGURE);
    assert_eq!(ours.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(ours.headers().get(header::RETRY_AFTER).unwrap(), "60");
    let theirs = crate::shop::market::unavailable(crate::shop::market::Source::TradesMirror, "x");
    let keys = |res: axum::response::Response| async move {
        let body: Value =
            serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 4096).await.unwrap())
                .unwrap();
        let mut keys: Vec<String> = body.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    assert_eq!(keys(ours).await, keys(theirs).await);
}

#[test]
fn settings_keep_the_old_defaults_for_unset_or_zero_knobs() {
    let s = Settings::new(false).with_knobs(0, Some("  "), Some(0));
    assert_eq!(s.snapshot_interval, std::time::Duration::from_secs(300));
    assert_eq!(s.genesis_realm, "main");
    assert_eq!(s.retention_days, None);
    let s = Settings::new(true).with_knobs(60, Some("test"), Some(30));
    assert_eq!(
        (
            s.writes_enabled,
            s.snapshot_interval.as_secs(),
            s.genesis_realm.as_str(),
            s.retention_days
        ),
        (true, 60, "test", Some(30))
    );
}

#[test]
fn the_schema_carries_the_snapshot_relations() {
    for table in [
        "snapshots",
        "peer_snapshots",
        "island_snapshots",
        "hot_scene_snapshots",
        "scene_occupancy",
        "world_membership",
        "daily_stats",
    ] {
        assert!(
            SCHEMA_SQL.contains(&format!("CREATE TABLE {SCHEMA}.{table} (")),
            "{table}"
        );
    }
}
