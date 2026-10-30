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
    async fn peers(&self) -> anyhow::Result<Vec<Peer>> {
        Ok(self.peers.clone())
    }
    async fn islands(&self) -> anyhow::Result<Vec<Island>> {
        Ok(self.islands.clone())
    }
    async fn hot_scenes(&self) -> anyhow::Result<Vec<HotScene>> {
        Ok(self.hot_scenes.clone())
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
    use super::super::sources::{
        HOT_SCENES_LIMIT, hot_scenes_of, island_of, peer_of, peers_per_tile,
    };
    use crate::comms::archipelago::content::Scene;
    use crate::comms::archipelago::feed::IslandReport;
    use crate::comms::archipelago::peers::PeerState;
    use std::collections::HashMap;

    fn peer_state(address: &str, x: f32, z: f32) -> PeerState {
        PeerState {
            address: address.into(),
            position: [x, 1.0, z],
            parcel: [0, 0],
            realm: "main".into(),
            last_heartbeat: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            island_id: None,
        }
    }

    fn scene(id: &str, base: [i32; 2], parcels: &[&str]) -> Scene {
        Scene {
            id: id.into(),
            name: Some(id.to_uppercase()),
            base,
            parcels: parcels.iter().map(|p| p.to_string()).collect(),
            thumbnail: None,
            creator: Some("0xc".into()),
            project_id: None,
            description: None,
        }
    }

    #[test]
    fn a_peer_row_carries_the_parcel_of_its_position_and_its_ping_in_ms() {
        let p = peer_of(&peer_state("0xAb", 96.5 * 16.0, -132.0 * 16.0));
        assert_eq!(
            (p.address.as_str(), p.parcel_x, p.parcel_y),
            ("0xAb", Some(96), Some(-132))
        );
        assert_eq!(
            (p.position_x, p.position_y, p.position_z),
            (Some(1544.0), Some(1.0), Some(-2112.0))
        );
        assert_eq!(p.last_ping, Some(1_700_000_000_000));
    }

    #[test]
    fn an_island_counts_only_the_members_this_replica_knows() {
        let known: HashMap<String, PeerState> = [("0xa".to_string(), peer_state("0xa", 0.0, 0.0))]
            .into_iter()
            .collect();
        let report = IslandReport {
            id: "I1".into(),
            peers: vec!["0xa".into(), "0xghost".into()],
            max_peers: 100,
            center: [1.0, 2.0, 3.0],
            radius: 50.0,
        };
        let i = island_of(&report, &known);
        assert_eq!(
            (i.island_id.as_str(), i.peer_count, i.max_peers),
            ("I1", 1, Some(100))
        );
        assert_eq!(
            (i.center_x, i.center_y, i.center_z, i.radius),
            (Some(1.0), Some(2.0), Some(3.0), Some(50.0))
        );
    }

    #[test]
    fn hot_scenes_sum_their_parcels_and_come_busiest_first() {
        let peers = vec![
            peer_state("a", 0.0, 0.0),
            peer_state("b", 5.0, 5.0),
            peer_state("c", 16.0, 0.0),
            peer_state("d", 800.0, 800.0),
        ];
        let per_tile = peers_per_tile(&peers);
        assert_eq!(per_tile.get("0,0"), Some(&2));
        assert_eq!(per_tile.get("1,0"), Some(&1));
        assert_eq!(per_tile.get("50,50"), Some(&1));
        let hot = hot_scenes_of(
            &per_tile,
            vec![
                scene("far", [50, 50], &["50,50"]),
                scene("plaza", [0, 0], &["0,0", "1,0", "2,0"]),
            ],
        );
        assert_eq!(
            hot.iter()
                .map(|h| (h.scene_id.as_str(), h.users_count, h.parcel_count))
                .collect::<Vec<_>>(),
            vec![("plaza", Some(3), 3), ("far", Some(1), 1)]
        );
        assert_eq!(
            (
                hot[0].base_x,
                hot[0].base_y,
                hot[0].name.as_deref(),
                hot[0].creator.as_deref()
            ),
            (Some(0), Some(0), Some("PLAZA"), Some("0xc"))
        );
        let many: Vec<Scene> = (0..HOT_SCENES_LIMIT + 5)
            .map(|i| scene(&format!("s{i}"), [i as i32, 0], &[]))
            .collect();
        assert_eq!(hot_scenes_of(&HashMap::new(), many).len(), HOT_SCENES_LIMIT);
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
