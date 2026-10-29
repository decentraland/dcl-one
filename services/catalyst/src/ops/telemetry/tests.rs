//! The old crate's contracts over the process's scratch database
//! (`db::scratch`; without `CATALYST_TEST_DATABASE_URL` the DB-backed tests
//! skip). The console reads are table-wide window scans, so the DB tests
//! take a process-local lock and start from empty telemetry tables (the
//! other lib tests never touch them); the `ops.admin_audit` rows they leave
//! are the telemetry domain's and are cleared the same way.

use std::sync::Arc;

use axum::Json;
use axum::body::{Body, Bytes, to_bytes};
use axum::extract::{FromRequestParts, Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use super::admin::{self, TelemetryAdmin};
use super::{
    Bounds, IngestControl, RawSettings, Settings, SharedState, State as TelemetryState,
    WRITER_JOB_NAME, dashboard, groups, purge, router, segment, sentry, sql,
};
use crate::http::Outbound;

/// Held by every test that writes the telemetry tables (these reset them).
pub(crate) static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Scratch {
    state: SharedState,
    pool: PgPool,
    _guard: tokio::sync::MutexGuard<'static, ()>,
}

/// The login `POST /dash/sql` runs on, provisioned as
/// `deploy/nix/postgresql.nix` provisions `catalyst_telemetry_reader`:
/// LOGIN NOINHERIT with no other attribute and no membership, the role's
/// timeout, read-only default and search path, CONNECT on the database,
/// USAGE on `telemetry` and SELECT on its tables. Cluster-wide under a
/// name of its own (the lab cluster's other tests never meet it), created
/// once by whichever test process gets there first, granted per scratch
/// database. The lab trusts local logins, so the password only exercises
/// the deployed DSN form.
const READER: &str = "catalyst_telemetry_sql_test";
const READER_PASSWORD: &str = "telemetry-sql-test";

async fn provision_reader(pool: &PgPool) {
    let database: String = sqlx::query_scalar("SELECT current_database()::text")
        .fetch_one(pool)
        .await
        .expect("current_database");
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DO $$ BEGIN \
           IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{READER}') THEN \
             CREATE ROLE {READER} LOGIN NOSUPERUSER NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS CONNECTION LIMIT 32 PASSWORD '{READER_PASSWORD}'; \
             ALTER ROLE {READER} SET statement_timeout = '15s'; \
             ALTER ROLE {READER} SET default_transaction_read_only = on; \
             ALTER ROLE {READER} SET search_path = telemetry, pg_catalog; \
           END IF; \
         EXCEPTION WHEN duplicate_object OR unique_violation THEN NULL; END $$; \
         GRANT CONNECT ON DATABASE \"{database}\" TO {READER}; \
         GRANT USAGE ON SCHEMA telemetry TO {READER}; \
         GRANT SELECT ON ALL TABLES IN SCHEMA telemetry TO {READER}; \
         REVOKE EXECUTE ON FUNCTION pg_catalog.pg_notify(text, text) FROM PUBLIC; \
         REVOKE EXECUTE ON FUNCTION pg_catalog.pg_logical_emit_message(boolean, text, text, boolean) FROM PUBLIC; \
         REVOKE EXECUTE ON FUNCTION pg_catalog.pg_logical_emit_message(boolean, text, bytea, boolean) FROM PUBLIC"
    )))
    .execute(pool)
    .await
    .expect("provisioning the telemetry reader login");
}

/// The reader's pool over the scratch database, from a DSN in the form the
/// deploy tree writes (`postgres:///<db>?host=<socket dir>&port=..&user=..&password=..`).
async fn reader_pool() -> Option<PgPool> {
    let url = crate::db::scratch::url().await?;
    let opts: sqlx::postgres::PgConnectOptions = url.parse().expect("the scratch URL");
    let host = opts
        .get_socket()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| opts.get_host().to_string());
    let database = opts
        .get_database()
        .expect("the scratch URL names its database");
    let dsn = format!(
        "postgres:///{database}?host={host}&port={}&user={READER}&password={READER_PASSWORD}",
        opts.get_port()
    );
    Some(
        sql::ReaderDsn::parse(&dsn)
            .expect("the deployed DSN form parses")
            .pool(),
    )
}

async fn reset(pool: &PgPool) {
    sqlx::raw_sql(
        "TRUNCATE telemetry.events, telemetry.issue_state, telemetry.issue_merges, telemetry.release_state, \
           telemetry.project_quotas, telemetry.experiment_overrides, telemetry.flag_overrides, \
           telemetry.flag_group_targets, telemetry.experiment_group_targets, telemetry.flag_groups, telemetry.product_areas; \
         UPDATE telemetry.ingest_settings SET enabled = true WHERE id = 1; \
         DELETE FROM ops.admin_audit WHERE domain = 'telemetry'",
    )
    .execute(pool)
    .await
    .expect("resetting the telemetry tables");
    provision_reader(pool).await;
}

fn settings(admin_token: Option<&str>) -> Settings {
    Settings {
        admin_token: admin_token.map(str::to_string),
        flags_url: Some("http://127.0.0.1:1/explorer.json".into()),
        ..Settings::for_tests()
    }
}

async fn scratch_with(
    settings: Settings,
    tune: impl FnOnce(&mut TelemetryState),
) -> Option<Scratch> {
    let guard = LOCK.lock().await;
    let pool = crate::db::scratch::pool(4).await?;
    reset(&pool).await;
    let mut state = TelemetryState::new(pool.clone(), Outbound::for_tests(), settings)
        .await
        .expect("telemetry state");
    state.sql_pool = reader_pool().await;
    tune(&mut state);
    state.start_writer_detached();
    Some(Scratch {
        state: Arc::new(state),
        pool,
        _guard: guard,
    })
}

async fn scratch(settings: Settings) -> Option<Scratch> {
    scratch_with(settings, |_| {}).await
}

async fn rows(pool: &PgPool) -> Vec<(String, String, String, Value)> {
    sqlx::query_as("SELECT source, project, event_kind, body FROM telemetry.events ORDER BY id")
        .fetch_all(pool)
        .await
        .expect("rows")
}

fn bytes(v: &Value) -> Bytes {
    Bytes::from(serde_json::to_vec(v).unwrap())
}

fn body<T: serde::de::DeserializeOwned>(v: Value) -> Json<T> {
    Json(serde_json::from_value(v).expect("body"))
}

fn q<T: serde::de::DeserializeOwned>(v: Value) -> Query<T> {
    Query(serde_json::from_value(v).expect("query"))
}

async fn admin_of(st: &SharedState) -> TelemetryAdmin {
    let (mut parts, ()) = axum::http::Request::builder()
        .header("authorization", "Bearer tok")
        .body(())
        .unwrap()
        .into_parts();
    TelemetryAdmin::from_request_parts(&mut parts, st)
        .await
        .expect("admin")
}

fn actor(name: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("x-catalyrst-admin", name.parse().unwrap());
    h
}

async fn audits(pool: &PgPool, action: &str) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT detail FROM ops.admin_audit WHERE domain = 'telemetry' AND action = $1 ORDER BY id",
    )
    .bind(action)
    .fetch_all(pool)
    .await
    .expect("audit rows")
}

async fn seed(pool: &PgPool, source: &str, kind: &str, secs_ago: i64, body: Value) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO telemetry.events (source, project, event_kind, received_at, body) \
         VALUES ($1, 'p', $2, timezone('UTC', now()) - make_interval(secs => $3::int), $4) RETURNING id",
    )
    .bind(source)
    .bind(kind)
    .bind(secs_ago)
    .bind(body)
    .fetch_one(pool)
    .await
    .expect("seed")
}

fn pairs(v: &Value) -> Vec<(String, i64)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_i64().unwrap()))
        .collect()
}

async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> (StatusCode, HeaderMap, String) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let req = match body {
        Some(b) => req.body(Body::from(b.to_string())).unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    (
        status,
        headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

#[tokio::test]
async fn batch_single_envelope_and_store_land_in_order() {
    let Some(s) = scratch(settings(None)).await else {
        return;
    };
    let st = s.state.clone();

    let batch = json!({ "writeKey": "wk", "batch": [
        { "type": "track", "event": "a", "properties": { "n": 1 } },
        { "type": "identify", "userId": "u1" },
        { "type": "track", "event": "b" },
    ]});
    let _ = segment::batch(State(st.clone()), HeaderMap::new(), bytes(&batch)).await;
    let _ = segment::single(
        State(st.clone()),
        HeaderMap::new(),
        bytes(&json!({ "writeKey": "wk", "type": "page", "name": "home" })),
    )
    .await;

    let envelope = concat!(
        "{\"event_id\":\"e1\"}\n",
        "{\"type\":\"event\"}\n",
        "{\"message\":\"boom\"}\n",
        "{\"type\":\"session\"}\n",
        "{\"status\":\"ok\"}\n",
    );
    let out = sentry::envelope(
        State(st.clone()),
        Path("proj".into()),
        HeaderMap::new(),
        Bytes::from_static(envelope.as_bytes()),
    )
    .await;
    assert_eq!(out.0["id"], json!("e1"));
    let _ = sentry::envelope(
        State(st.clone()),
        Path("proj".into()),
        HeaderMap::new(),
        Bytes::from_static(b"{\"event_id\":\"e2\"}\n"),
    )
    .await;
    let _ = sentry::store(
        State(st.clone()),
        Path("proj".into()),
        HeaderMap::new(),
        bytes(&json!({ "event_id": "e3", "message": "stored" })),
    )
    .await;

    st.writer.flush().await;
    let got = rows(&s.pool).await;
    let shape: Vec<(&str, &str, &str)> = got
        .iter()
        .map(|(s, p, k, _)| (s.as_str(), p.as_str(), k.as_str()))
        .collect();
    assert_eq!(
        shape,
        vec![
            ("segment", "wk", "track"),
            ("segment", "wk", "identify"),
            ("segment", "wk", "track"),
            ("segment", "wk", "page"),
            ("sentry", "proj", "event"),
            ("sentry", "proj", "session"),
            ("sentry", "proj", "envelope"),
            ("sentry", "proj", "event"),
        ]
    );
    assert_eq!(got[0].3["properties"]["n"], json!(1));
    assert_eq!(got[4].3["message"], json!("boom"));
    assert!(got[6].3["raw"].as_str().unwrap().contains("e2"));
    assert_eq!(got[7].3["event_id"], json!("e3"));
    let fingerprints: Vec<String> =
        sqlx::query_scalar("SELECT fingerprint FROM telemetry.events ORDER BY id")
            .fetch_all(&s.pool)
            .await
            .unwrap();
    assert_eq!(fingerprints[0], "a");
    assert_eq!(
        fingerprints[1], "identify uN",
        "the STORED fingerprint normalises numbers"
    );
    assert_eq!(fingerprints[5], "session (ok)");
}

#[tokio::test]
async fn a_bad_row_only_loses_itself() {
    let Some(s) = scratch(settings(None)).await else {
        return;
    };
    let st = s.state.clone();

    let batch = json!({ "writeKey": "wk", "batch": [
        { "type": "track", "event": "ok1" },
        { "type": "track", "event": "nul\u{0}byte" },
        { "type": "track", "event": "ok2" },
    ]});
    let _ = segment::batch(State(st.clone()), HeaderMap::new(), bytes(&batch)).await;
    let _ = segment::single(
        State(st.clone()),
        HeaderMap::new(),
        bytes(&json!({ "writeKey": "wk", "type": "track", "event": "bad\u{0}" })),
    )
    .await;
    let _ = segment::single(
        State(st.clone()),
        HeaderMap::new(),
        bytes(&json!({ "writeKey": "wk", "type": "track", "event": "ok3" })),
    )
    .await;
    st.writer.flush().await;

    let events: Vec<String> = rows(&s.pool)
        .await
        .into_iter()
        .map(|(_, _, _, b)| b["event"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(events, vec!["ok1", "ok2", "ok3"]);
}

#[tokio::test]
async fn batches_preserve_quotas_contract_flags_and_valid_neighbors() {
    let Some(s) = scratch_with(settings(None), |state| {
        state.contract = Some(Arc::new(
            serde_json::from_value(
                json!({"events":{"tour":{"props":{"parcel":{"kind":"string"}}}}}),
            )
            .unwrap(),
        ));
    })
    .await
    else {
        return;
    };
    let st = s.state.clone();
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS telemetry.insert_calls (n int); \
         CREATE OR REPLACE FUNCTION telemetry.count_ingest() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN INSERT INTO telemetry.insert_calls VALUES (1); RETURN NULL; END $$; \
         CREATE OR REPLACE TRIGGER count_ingest AFTER INSERT ON telemetry.events FOR EACH STATEMENT EXECUTE FUNCTION telemetry.count_ingest();",
    )
    .execute(&s.pool)
    .await
    .unwrap();
    // 600 events over six requests of MAX_ITEMS_PER_REQUEST (a seventh
    // event in one request would be cut, not queued), under a 513 quota.
    st.ingest
        .quotas
        .write()
        .unwrap()
        .insert("segment-test".into(), 513);
    for page in 0..6 {
        let batch: Vec<Value> = (page * 100..(page + 1) * 100)
            .map(|i| json!({"event":"tour", "properties":{"parcel": if i == 1 {json!(false)} else {json!("0,0")}}, "i":i}))
            .collect();
        let response = segment::batch(
            State(st.clone()),
            HeaderMap::new(),
            Bytes::from(json!({"writeKey":"segment-test","batch":batch}).to_string()),
        )
        .await;
        assert_eq!(response.0, json!({"success":true}));
    }
    st.writer.flush().await;
    let (count, invalid, last): (i64, i64, i32) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE invalid_reason IS NOT NULL), max((body->>'i')::int) FROM telemetry.events",
    )
    .fetch_one(&s.pool)
    .await
    .unwrap();
    assert_eq!((count, invalid, last), (513, 1, 512));
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM telemetry.insert_calls")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(
        calls, 3,
        "513 rows must take three bounded insert statements"
    );
    let _ = segment::batch(
        State(st.clone()),
        HeaderMap::new(),
        Bytes::from(json!({"writeKey":"segment-test","batch":[{"i":999}]}).to_string()),
    )
    .await;
    st.writer.flush().await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM telemetry.events")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(count, 513, "the quota is spent for the day");

    sqlx::query("ALTER TABLE telemetry.events ADD CONSTRAINT reject_bad_event CHECK (body->>'reject' IS DISTINCT FROM 'yes')")
        .execute(&s.pool)
        .await
        .unwrap();
    let _ = segment::batch(
        State(st.clone()),
        HeaderMap::new(),
        Bytes::from(
            json!({"writeKey":"fallback", "batch":[{"ok":1},{"reject":"yes"},{"ok":2}]})
                .to_string(),
        ),
    )
    .await;
    st.writer.flush().await;
    assert_eq!(
        st.writer.refused.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "the refused row is counted"
    );
    let bodies: Vec<Value> = sqlx::query_scalar(
        "SELECT body FROM telemetry.events WHERE project='fallback' ORDER BY id",
    )
    .fetch_all(&s.pool)
    .await
    .unwrap();
    assert_eq!(bodies, vec![json!({"ok":1}), json!({"ok":2})]);

    st.ingest
        .quotas
        .write()
        .unwrap()
        .insert("sentry-test".into(), 2);
    let envelope = "{\"event_id\":\"abc\"}\n{\"type\":\"event\"}\n{\"message\":\"one\"}\n{\"type\":\"session\"}\n{\"status\":\"ok\"}\n{\"type\":\"event\"}\n{\"message\":\"excluded\"}\n";
    let response = sentry::envelope(
        State(st.clone()),
        Path("sentry-test".into()),
        HeaderMap::new(),
        Bytes::from(envelope),
    )
    .await;
    assert_eq!(response.0, json!({"id":"abc"}));
    st.writer.flush().await;
    let kinds: Vec<String> = sqlx::query_scalar(
        "SELECT event_kind FROM telemetry.events WHERE project='sentry-test' ORDER BY id",
    )
    .fetch_all(&s.pool)
    .await
    .unwrap();
    assert_eq!(kinds, vec!["event", "session"]);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM telemetry.insert_calls")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(
        calls, 6,
        "three batch inserts, two fallback successes, one Sentry batch"
    );
    sqlx::raw_sql(
        "ALTER TABLE telemetry.events DROP CONSTRAINT reject_bad_event; \
         DROP TRIGGER count_ingest ON telemetry.events; DROP FUNCTION telemetry.count_ingest(); DROP TABLE telemetry.insert_calls",
    )
    .execute(&s.pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn the_toggle_and_the_quotas_are_seeded_from_the_tables_at_boot() {
    let Some(s) = scratch(settings(None)).await else {
        return;
    };
    sqlx::raw_sql(
        "INSERT INTO telemetry.project_quotas (project, daily_limit) VALUES ('capped', 2); \
         UPDATE telemetry.ingest_settings SET enabled = false WHERE id = 1",
    )
    .execute(&s.pool)
    .await
    .unwrap();
    let again = TelemetryState::new(s.pool.clone(), Outbound::for_tests(), settings(None))
        .await
        .unwrap();
    assert_eq!(again.ingest.quotas.read().unwrap().get("capped"), Some(&2));
    assert_eq!(
        again.ingest.admit_n("capped", 5),
        0,
        "the persisted kill switch is loaded"
    );
    again
        .ingest
        .enabled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(again.ingest.admit_n("capped", 5), 2);
    assert_eq!(again.ingest.admit_n("capped", 5), 0);
    assert_eq!(
        again.ingest.admit_n("free", 5),
        5,
        "no row means the default limit (unlimited under the test bounds)"
    );
    let fresh = IngestControl::new(Bounds::unlimited());
    assert_eq!(fresh.admit_n("x", 0), 0);
    assert!(fresh.admit("x"));
}

/// The bounds that hold without any operator row: an unknown project takes
/// the default daily limit (or the `*` row), the process-wide rows ceiling
/// grants what is left of the minute, the bytes ceiling refuses a request
/// whole, and every refusal is counted.
#[test]
fn unknown_projects_are_capped_and_the_ceilings_hold() {
    let control = IngestControl::new(Bounds {
        default_daily_limit: 3,
        max_rows_per_minute: 5,
        max_bytes_per_minute: 10,
    });
    assert_eq!(
        control.admit_n("rotating-1", 5),
        3,
        "the default daily limit, not the request size"
    );
    assert_eq!(
        control.admit_n("rotating-2", 5),
        2,
        "the rows ceiling grants the remainder of the minute"
    );
    assert_eq!(
        control.admit_n("rotating-3", 1),
        0,
        "the minute is spent for every project"
    );
    assert_eq!(
        control
            .refused_rows
            .load(std::sync::atomic::Ordering::Relaxed),
        2 + 3 + 1
    );
    assert!(
        !control.admit_bytes(11),
        "a request past the bytes ceiling stores nothing"
    );
    assert!(control.admit_bytes(10));
    assert!(!control.admit_bytes(1));
    assert_eq!(
        control
            .refused_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        2
    );

    let starred = IngestControl::new(Bounds {
        default_daily_limit: 3,
        max_rows_per_minute: u32::MAX,
        max_bytes_per_minute: u64::MAX,
    });
    starred.quotas.write().unwrap().insert("*".into(), 1);
    starred.quotas.write().unwrap().insert("named".into(), 4);
    assert_eq!(
        starred.daily_limit("anything"),
        1,
        "the * row stands in for the default"
    );
    assert_eq!(starred.daily_limit("named"), 4, "an own row wins over *");
    assert_eq!(starred.admit_n("anything", 5), 1);
    assert_eq!(starred.admit_n("named", 5), 4);

    let closed = IngestControl::new(Bounds {
        default_daily_limit: 0,
        max_rows_per_minute: u32::MAX,
        max_bytes_per_minute: u64::MAX,
    });
    assert_eq!(
        closed.admit_n("stranger", 1),
        0,
        "a zero default refuses every unnamed project"
    );
}

/// The sinks under the bounds: rotating project names store at most the
/// ceiling, an envelope keeps at most MAX_ITEMS_PER_REQUEST items, an
/// oversize request stores nothing, and the no-item envelope keeps a bounded
/// prefix rather than the body.
#[tokio::test]
async fn a_flood_of_rotating_projects_is_bounded() {
    let bounded = Settings {
        bounds: Bounds {
            default_daily_limit: 5,
            max_rows_per_minute: 12,
            max_bytes_per_minute: 64 * 1024,
        },
        ..settings(None)
    };
    let Some(s) = scratch(bounded).await else {
        return;
    };
    let st = s.state.clone();
    let item = "{\"type\":\"event\"}\n{\"message\":\"m\"}\n";
    let three = format!("{{\"event_id\":\"e\"}}\n{item}{item}{item}");
    for i in 0..10 {
        let out = sentry::envelope(
            State(st.clone()),
            Path(format!("rotating-{i}")),
            HeaderMap::new(),
            Bytes::from(three.clone()),
        )
        .await;
        assert_eq!(
            out.0,
            json!({ "id": "e" }),
            "the success shape whatever was stored"
        );
    }
    st.writer.flush().await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM telemetry.events")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(
        count, 12,
        "30 items over 10 unnamed projects: the rows ceiling holds at 12"
    );
    assert_eq!(
        st.ingest
            .refused_rows
            .load(std::sync::atomic::Ordering::Relaxed),
        18
    );

    let many = format!("{{\"event_id\":\"big\"}}\n{}", item.repeat(150));
    let _ = sentry::envelope(
        State(st.clone()),
        Path("big".into()),
        HeaderMap::new(),
        Bytes::from(many.clone()),
    )
    .await;
    st.writer.flush().await;
    let big: i64 =
        sqlx::query_scalar("SELECT count(*) FROM telemetry.events WHERE project = 'big'")
            .fetch_one(&s.pool)
            .await
            .unwrap();
    assert_eq!(big, 0, "the minute is spent");

    let unbounded = Settings {
        bounds: Bounds {
            default_daily_limit: i64::MAX,
            max_rows_per_minute: u32::MAX,
            max_bytes_per_minute: 64 * 1024,
        },
        ..settings(None)
    };
    let st = Arc::new(
        TelemetryState::new(s.pool.clone(), Outbound::for_tests(), unbounded)
            .await
            .unwrap(),
    );
    st.start_writer_detached();
    let _ = sentry::envelope(
        State(st.clone()),
        Path("big".into()),
        HeaderMap::new(),
        Bytes::from(many),
    )
    .await;
    let oversize = format!(
        "{{\"event_id\":\"o\"}}\n{{\"type\":\"event\"}}\n{{\"message\":\"{}\"}}\n",
        "x".repeat(65 * 1024)
    );
    let out = sentry::envelope(
        State(st.clone()),
        Path("over".into()),
        HeaderMap::new(),
        Bytes::from(oversize),
    )
    .await;
    assert_eq!(
        out.0,
        json!({ "id": "" }),
        "a request past the bytes ceiling answers the empty id"
    );
    let junk = format!("{{\"event_id\":\"j\"}}\n{}\n", "not json ".repeat(1000));
    let _ = sentry::envelope(
        State(st.clone()),
        Path("junk".into()),
        HeaderMap::new(),
        Bytes::from(junk),
    )
    .await;
    let batch: Vec<Value> = (0..150)
        .map(|i| json!({ "type": "track", "event": "e", "i": i }))
        .collect();
    let _ = segment::batch(
        State(st.clone()),
        HeaderMap::new(),
        Bytes::from(json!({ "writeKey": "wide", "batch": batch }).to_string()),
    )
    .await;
    st.writer.flush().await;
    let per_project: Vec<(String, i64)> =
        sqlx::query_as("SELECT project, count(*) FROM telemetry.events WHERE project IN ('big', 'over', 'junk', 'wide') GROUP BY project ORDER BY project")
            .fetch_all(&s.pool)
            .await
            .unwrap();
    assert_eq!(
        per_project,
        vec![
            ("big".into(), 100),
            ("junk".into(), 1),
            ("wide".into(), 100)
        ],
        "100 items per request, nothing for the oversize one"
    );
    let (raw_len, raw_bytes, truncated): (i32, i64, bool) =
        sqlx::query_as("SELECT length(body->>'raw'), (body->>'raw_bytes')::bigint, (body->>'truncated')::boolean FROM telemetry.events WHERE project = 'junk'")
            .fetch_one(&s.pool)
            .await
            .unwrap();
    assert_eq!(
        (raw_len, truncated),
        (super::MAX_RAW_ENVELOPE_BYTES as i32, true)
    );
    assert!(raw_bytes > 9000, "{raw_bytes}");
    assert_eq!(
        st.ingest
            .refused_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    let counters = st.ingest_counters();
    assert_eq!(counters["written"], json!(201));
    assert_eq!(counters["refused_requests"], json!(1));
    assert_eq!(counters["bounds"]["max_items_per_request"], json!(100));
}

/// The writer is the job body: a full queue drops (and counts) rather than
/// inserting from the request, the job report names it while it runs, and
/// shutdown drains what was queued.
#[tokio::test]
async fn the_writer_is_a_job_that_drops_when_full_and_drains_at_shutdown() {
    let guard = LOCK.lock().await;
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    reset(&pool).await;
    let state = Arc::new(
        TelemetryState::new(pool.clone(), Outbound::for_tests(), settings(None))
            .await
            .unwrap(),
    );
    let row = |i: usize| super::ingest::Row {
        source: "segment",
        project: "q".into(),
        kind: "track".into(),
        body: json!({ "i": i }),
        invalid_reason: None,
    };
    for i in 0..super::ingest::QUEUE_CAP {
        assert!(state.writer.push(row(i)), "row {i} fits the queue");
    }
    assert!(
        !state.writer.push(row(super::ingest::QUEUE_CAP)),
        "the queue is full: the row is dropped, not inserted inline"
    );
    assert_eq!(
        state
            .writer
            .dropped
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );

    let jobs = crate::jobs::Jobs::new();
    state.spawn_jobs(&jobs);
    let names: Vec<String> = jobs.report().into_iter().map(|j| j.name).collect();
    assert!(
        names.contains(&WRITER_JOB_NAME.to_string())
            && names.contains(&super::PURGE_JOB_NAME.to_string()),
        "{names:?}"
    );
    state.writer.flush().await;
    let progress = jobs
        .report()
        .into_iter()
        .find(|j| j.name == WRITER_JOB_NAME)
        .and_then(|j| j.progress)
        .unwrap_or_default();
    assert!(
        progress.starts_with("written 10000 refused 0"),
        "{progress}"
    );
    assert!(state.writer.push(row(1)), "room again after the drain");
    let aborted = jobs.shutdown(std::time::Duration::from_secs(30)).await;
    assert!(aborted.is_empty(), "{aborted:?}");
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM telemetry.events WHERE project = 'q'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 10_001, "shutdown wrote the queued row");
    assert!(
        !state.writer.push(row(2)),
        "after shutdown every push is a counted drop"
    );
    assert_eq!(
        state
            .writer
            .dropped
            .load(std::sync::atomic::Ordering::Relaxed),
        2
    );
    drop(guard);
}

#[tokio::test]
async fn merged_dashboards_keep_their_shapes() {
    let Some(s) = scratch(settings(None)).await else {
        return;
    };
    let st = s.state.clone();
    let pool = &s.pool;

    let ev = |user: &str, start: &str, level: &str, msg: &str| {
        json!({ "message": msg, "level": level, "environment": "prod", "release": "r1",
                "user": { "id": user }, "contexts": { "app": { "app_start_time": start } } })
    };
    let a1 = seed(pool, "sentry", "event", 30, ev("u1", "A", "error", "e1")).await;
    seed(pool, "sentry", "event", 20, ev("u1", "A", "info", "i1")).await;
    seed(pool, "sentry", "event", 10, ev("u1", "A", "fatal", "f1")).await;
    seed(pool, "sentry", "event", 25, ev("u1", "B", "info", "i2")).await;
    let u2 = seed(pool, "sentry", "event", 5, ev("u2", "A", "warning", "w1")).await;
    let sess = |status: &str, did: &str, rel: &str| json!({ "status": status, "did": did, "attrs": { "release": rel } });
    let s1 = seed(pool, "sentry", "session", 40, sess("ok", "d1", "r1")).await;
    seed(pool, "sentry", "session", 39, sess("crashed", "d1", "r1")).await;
    seed(pool, "sentry", "session", 38, sess("exited", "d2", "r1")).await;
    seed(pool, "sentry", "session", 37, sess("", "d2", "r2")).await;

    let x1 = seed(
        pool,
        "segment",
        "track",
        60,
        json!({ "event": "a", "userId": "x", "properties": {} }),
    )
    .await;
    seed(
        pool,
        "segment",
        "track",
        50,
        json!({ "event": "b", "userId": "x", "context": { "campaign": { "name": "old" } } }),
    )
    .await;
    seed(
        pool,
        "segment",
        "track",
        40,
        json!({ "event": "a", "userId": "y", "properties": { "campaign": { "name": "new" } } }),
    )
    .await;
    seed(pool, "segment", "identify", 30, json!({ "userId": "x" })).await;
    let orphan = seed(pool, "segment", "page", 20, json!({ "name": "home" })).await;
    seed(
        pool,
        "segment",
        "track",
        3600 * 30,
        json!({ "event": "a", "userId": "x" }),
    )
    .await;

    let stats = dashboard::stats(State(st.clone()), q(json!({ "hours": 24 })))
        .await
        .expect("stats")
        .0;
    assert_eq!(stats["total"], json!(14));
    assert_eq!(stats["bucket"], json!("hour"));
    assert_eq!(
        pairs(&stats["by_source"]),
        vec![("sentry".into(), 9), ("segment".into(), 5)]
    );
    assert_eq!(
        pairs(&stats["by_kind"])[0],
        ("event".to_string(), 5),
        "{}",
        stats["by_kind"]
    );
    assert_eq!(pairs(&stats["by_env"]), vec![("prod".into(), 5)]);
    assert_eq!(pairs(&stats["by_release"]), vec![("r1".into(), 5)]);
    let levels = pairs(&stats["by_level"]);
    assert_eq!(levels[0], ("(none)".to_string(), 9));
    assert_eq!(levels.iter().map(|(_, c)| c).sum::<i64>(), 14);
    assert_eq!(
        pairs(&stats["series"]).iter().map(|(_, c)| c).sum::<i64>(),
        14
    );
    let scoped = dashboard::stats(
        State(st.clone()),
        q(json!({ "hours": 24, "source": "segment" })),
    )
    .await
    .expect("stats scoped")
    .0;
    assert_eq!(scoped["total"], json!(5));
    assert_eq!(scoped["by_env"], json!([]));

    let health = dashboard::health(State(st.clone()), q(json!({ "hours": 24 })))
        .await
        .expect("health")
        .0;
    assert_eq!(health["total"], json!(4));
    assert_eq!(health["crashed"], json!(1));
    assert_eq!(health["unhealthy"], json!(1));
    assert_eq!(health["total_users"], json!(2));
    assert_eq!(health["crashed_users"], json!(1));
    assert_eq!(health["crash_free_rate"], json!(75.0));
    assert_eq!(health["crash_free_users_rate"], json!(50.0));
    assert_eq!(pairs(&health["by_status"])[0], ("ok".to_string(), 2));
    assert_eq!(health["by_release"][0]["release"], json!("r1"));
    assert_eq!(health["by_release"][0]["sessions"], json!(3));
    assert_eq!(health["by_release"][1]["release"], json!("r2"));
    assert_eq!(health["by_release"][1]["crash_free"], json!(100.0));
    assert_eq!(
        pairs(&health["series"]).iter().map(|(_, c)| c).sum::<i64>(),
        4
    );
    let filtered = dashboard::health(
        State(st.clone()),
        q(json!({ "hours": 24, "release": "r2" })),
    )
    .await
    .expect("health r2")
    .0;
    assert_eq!(filtered["total"], json!(1));
    assert_eq!(filtered["total_users"], json!(1));

    let metrics = dashboard::metrics(State(st.clone()), q(json!({ "hours": 24 })))
        .await
        .expect("metrics")
        .0;
    assert_eq!(metrics["total"], json!(5));
    assert_eq!(metrics["users"], json!(2));
    assert_eq!(
        pairs(&metrics["by_event"]),
        vec![("a".into(), 2), ("b".into(), 1)]
    );
    assert_eq!(pairs(&metrics["by_type"])[0], ("track".to_string(), 3));
    assert_eq!(
        pairs(&metrics["series"])
            .iter()
            .map(|(_, c)| c)
            .sum::<i64>(),
        5
    );

    let sess_a = dashboard::session(State(st.clone()), Path(a1))
        .await
        .expect("session")
        .0;
    assert_eq!(sess_a["user"], json!("u1"));
    assert_eq!(sess_a["app_start"], json!("A"));
    assert_eq!(sess_a["anchor"], json!(a1));
    assert_eq!(sess_a["total"], json!(3));
    assert_eq!(sess_a["errors"], json!(2));
    assert!(sess_a["first"].as_str().unwrap() <= sess_a["last"].as_str().unwrap());
    let titles: Vec<&str> = sess_a["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["e1", "i1", "f1"]);
    assert_eq!(sess_a["events"][0]["id"], json!(a1));
    assert_eq!(sess_a["events"][0]["kind"], json!("event"));
    assert_eq!(pairs(&sess_a["by_level"]).len(), 3);
    let sess_u2 = dashboard::session(State(st.clone()), Path(u2))
        .await
        .expect("session u2")
        .0;
    assert_eq!(sess_u2["total"], json!(1));
    let sess_none = dashboard::session(State(st.clone()), Path(s1))
        .await
        .expect("session without user")
        .0;
    assert_eq!(sess_none, json!({ "user": null, "events": [] }));
    let sess_missing = dashboard::session(State(st.clone()), Path(-1))
        .await
        .expect("session missing")
        .0;
    assert_eq!(sess_missing, json!({ "user": null, "events": [] }));

    let story = dashboard::story(State(st.clone()), Path(x1))
        .await
        .expect("story")
        .0;
    assert_eq!(story["user"], json!("x"));
    assert_eq!(story["count"], json!(3));
    assert_eq!(story["utm"], json!({ "name": "old" }));
    let events = story["events"].as_array().unwrap();
    assert_eq!(events[0]["id"], json!(x1));
    assert_eq!(events[0]["current"], json!(true));
    assert_eq!(events[1]["current"], json!(false));
    assert_eq!(events[1]["kind"], json!("track"));
    assert_eq!(events[2]["kind"], json!("identify"));
    assert_eq!(events[0]["source"], json!("segment"));
    let story_orphan = dashboard::story(State(st.clone()), Path(orphan))
        .await
        .expect("story orphan")
        .0;
    assert_eq!(
        story_orphan,
        json!({ "user": null, "utm": null, "events": [] })
    );
    let missing = dashboard::story(State(st.clone()), Path(-1)).await;
    assert_eq!(missing.err().map(|e| e.0), Some(StatusCode::NOT_FOUND));

    // The events page and the issues aggregate over the same window.
    let list = dashboard::events(
        State(st.clone()),
        q(json!({ "hours": 24, "source": "sentry", "kind": "event" })),
    )
    .await
    .expect("events")
    .0;
    assert_eq!(list["group"], json!(false));
    assert_eq!(list["items"].as_array().unwrap().len(), 5);
    assert_eq!(list["items"][0]["title"], json!("w1"), "newest first");
    let issues = dashboard::events(State(st.clone()), q(json!({ "hours": 24, "source": "sentry", "kind": "event", "group": 1, "sort": "frequent" })))
        .await
        .expect("issues")
        .0;
    assert_eq!(issues["group"], json!(true));
    let items = issues["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        4,
        "four distinct fingerprints: i1 and i2 normalise to iN"
    );
    assert_eq!(
        (
            items[0]["fingerprint"].as_str(),
            items[0]["count"].as_i64(),
            items[0]["users"].as_i64()
        ),
        (Some("iN"), Some(2), Some(1))
    );
    assert!(items[1..].iter().all(|i| i["status"] == json!("unresolved")
        && i["count"] == json!(1)
        && i["users"] == json!(1)));
    let (mut parts, ()) = axum::http::Request::builder()
        .header("authorization", "Bearer tok")
        .body(())
        .unwrap()
        .into_parts();
    assert!(
        TelemetryAdmin::from_request_parts(&mut parts, &st)
            .await
            .is_err(),
        "no admin token in this state: the extractor refuses"
    );
    sqlx::query("INSERT INTO telemetry.issue_state (fingerprint, status, updated_at) VALUES ('iN', 'resolved', timezone('UTC', now()) - interval '1 hour')")
        .execute(pool)
        .await
        .unwrap();
    let issues = dashboard::events(State(st.clone()), q(json!({ "hours": 24, "source": "sentry", "kind": "event", "group": 1, "sort": "frequent" })))
        .await
        .expect("issues again")
        .0;
    assert_eq!(
        issues["items"][0]["status"],
        json!("unresolved"),
        "a resolved issue with a newer event reads as regressed"
    );
    let only_unresolved = dashboard::events(State(st.clone()), q(json!({ "hours": 24, "source": "sentry", "kind": "event", "group": 1, "status": "resolved" })))
        .await
        .expect("issues by status")
        .0;
    assert_eq!(only_unresolved["items"].as_array().unwrap().len(), 0);
    let searched = dashboard::events(State(st.clone()), q(json!({ "hours": 24, "q": "f1" })))
        .await
        .expect("search")
        .0;
    assert_eq!(searched["items"].as_array().unwrap().len(), 1);
    let by_tag = dashboard::events(
        State(st.clone()),
        q(json!({ "hours": 24, "prop": "campaign:x" })),
    )
    .await
    .expect("prop")
    .0;
    assert_eq!(by_tag["items"].as_array().unwrap().len(), 0);
    let detail = dashboard::event_detail(State(st.clone()), Path(a1))
        .await
        .expect("detail")
        .0;
    assert_eq!(detail["body"]["message"], json!("e1"));
    assert!(detail["received_at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(
        dashboard::event_detail(State(st.clone()), Path(-1))
            .await
            .err()
            .map(|e| e.0),
        Some(StatusCode::NOT_FOUND)
    );

    let funnel = dashboard::funnel(State(st.clone()), q(json!({ "hours": 24, "steps": "a|b" })))
        .await
        .expect("funnel")
        .0;
    assert_eq!(funnel["steps"][0]["users"], json!(2));
    assert_eq!(funnel["steps"][1]["users"], json!(1));
    assert_eq!(
        dashboard::funnel(State(st.clone()), q(json!({ "steps": "a" })))
            .await
            .err()
            .map(|e| e.0),
        Some(StatusCode::BAD_REQUEST)
    );
    let keys = dashboard::breakdown(State(st.clone()), q(json!({ "hours": 24, "event": "a" })))
        .await
        .expect("breakdown keys")
        .0;
    assert_eq!(keys["props"], json!(["campaign"]));
    let broken = dashboard::breakdown(
        State(st.clone()),
        q(json!({ "hours": 24, "event": "a", "prop": "campaign" })),
    )
    .await
    .expect("breakdown")
    .0;
    assert_eq!(broken["rows"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn flag_override_roundtrip_merge_clear_and_audit() {
    let Some(s) = scratch(settings(Some("tok"))).await else {
        return;
    };
    let st = s.state.clone();

    let _ = dashboard::flag_set(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "flag": "gv_banner", "state": "forced", "variant": "guided" })),
    )
    .await
    .expect("flag_set");

    let row: (String, Option<String>) = sqlx::query_as(
        "SELECT state, forced_variant FROM telemetry.flag_overrides WHERE flag = $1",
    )
    .bind("gv_banner")
    .fetch_one(&s.pool)
    .await
    .expect("override row");
    assert_eq!(row.0, "forced");
    assert_eq!(row.1.as_deref(), Some("guided"));

    let flags = dashboard::flags(
        State(st.clone()),
        Query(dashboard::FlagsQuery { user: None }),
    )
    .await
    .expect("flags")
    .0;
    let entry = &flags["flags"]["gv_banner"];
    assert_eq!(entry["value"], json!(true));
    assert_eq!(entry["variant"], json!("guided"));
    assert_eq!(entry["overridden"], json!(true));
    assert_eq!(entry["override_state"], json!("forced"));
    assert_eq!(flags["overrides"]["gv_banner"]["state"], json!("forced"));
    assert_eq!(
        flags["config"],
        Value::Null,
        "an unreachable flags source is null, not an error"
    );
    assert_eq!(
        flags["source_url"],
        json!("http://127.0.0.1:1/explorer.json")
    );

    let (sets,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM ops.admin_audit WHERE domain = 'telemetry' AND action = 'flag.set' AND target = 'gv_banner' AND detail->>'flag' = 'gv_banner'",
    )
    .fetch_one(&s.pool)
    .await
    .expect("set audit count");
    assert_eq!(sets, 1);

    let _ = dashboard::flag_set(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "flag": "gv_banner", "clear": true })),
    )
    .await
    .expect("flag clear");

    let (remaining,): (i64,) = sqlx::query_as("SELECT count(*) FROM telemetry.flag_overrides")
        .fetch_one(&s.pool)
        .await
        .expect("remaining count");
    assert_eq!(remaining, 0);

    let flags2 = dashboard::flags(
        State(st.clone()),
        Query(dashboard::FlagsQuery { user: None }),
    )
    .await
    .expect("flags2")
    .0;
    assert!(flags2["flags"].get("gv_banner").is_none());

    let (clears,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM ops.admin_audit WHERE action = 'flag.clear' AND detail->>'flag' = 'gv_banner'")
            .fetch_one(&s.pool)
            .await
            .expect("clear audit count");
    assert_eq!(clears, 1);
}

#[tokio::test]
async fn groups_flags_and_experiments_fold_with_audit() {
    let Some(s) = scratch(settings(Some("tok"))).await else {
        return;
    };
    let st = s.state.clone();
    let pool = &s.pool;

    for g in [
        json!({ "name": "beta", "members": ["u1"], "priority": 10 }),
        json!({ "name": "alpha", "members": ["u1", "u2"], "priority": 5, "description": "d" }),
    ] {
        let _ = groups::set(admin_of(&st).await, State(st.clone()), body(g))
            .await
            .expect("set");
    }
    for t in [
        json!({ "group": "beta", "flag": "f1", "state": "forced", "variant": "vb" }),
        json!({ "group": "alpha", "flag": "f1", "state": "off" }),
        json!({ "group": "alpha", "flag": "f2" }),
        json!({ "group": "alpha", "exp_key": "e1", "variant": "va" }),
        json!({ "group": "beta", "exp_key": "e1", "killed": true }),
    ] {
        let _ = groups::set_target(admin_of(&st).await, State(st.clone()), body(t))
            .await
            .expect("set_target");
    }
    let _ = groups::set_area(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "kind": "flag", "name": "f1", "area": "core" })),
    )
    .await
    .expect("set_area");
    let both = groups::set_target(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "group": "beta", "flag": "f1", "exp_key": "e1" })),
    )
    .await;
    assert_eq!(both.err().map(|e| e.0), Some(StatusCode::BAD_REQUEST));

    let list = groups::list(State(st.clone())).await.expect("list").0;
    let names: Vec<&str> = list["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["beta", "alpha"]);
    let alpha = &list["groups"][1];
    assert_eq!(alpha["member_count"], json!(2));
    assert_eq!(alpha["members"], json!(["u1", "u2"]));
    assert_eq!(alpha["description"], json!("d"));
    assert_eq!(
        alpha["flag_targets"],
        json!([
            { "flag": "f1", "state": "off", "variant": null },
            { "flag": "f2", "state": "on", "variant": null },
        ])
    );
    assert_eq!(
        alpha["experiment_targets"],
        json!([{ "exp_key": "e1", "killed": false, "variant": "va" }])
    );
    assert_eq!(
        list["groups"][0]["flag_targets"],
        json!([{ "flag": "f1", "state": "forced", "variant": "vb" }])
    );
    assert_eq!(
        list["areas"],
        json!([{ "kind": "flag", "name": "f1", "area": "core" }])
    );
    assert_eq!(audits(pool, "group.set").await.len(), 2);
    assert_eq!(audits(pool, "group.target.set").await.len(), 5);
    assert_eq!(audits(pool, "area.set").await[0]["area"], json!("core"));

    let f = |user: Option<&str>| {
        dashboard::flags(
            State(st.clone()),
            Query(dashboard::FlagsQuery {
                user: user.map(String::from),
            }),
        )
    };
    let u1 = f(Some("u1")).await.expect("flags u1").0;
    assert_eq!(u1["config"], Value::Null);
    assert_eq!(u1["user"], json!("u1"));
    assert_eq!(u1["groups"], json!(["beta", "alpha"]));
    assert_eq!(u1["overrides"]["f1"]["state"], json!("forced"));
    assert_eq!(u1["overrides"]["f1"]["variant"], json!("vb"));
    assert_eq!(u1["overrides"]["f2"]["state"], json!("on"));
    let mut targeted: Vec<&str> = u1["group_targeted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    targeted.sort();
    assert_eq!(targeted, vec!["f1", "f2"]);
    assert_eq!(u1["areas"], json!({ "f1": "core" }));
    assert_eq!(u1["observed"], json!([]));
    let u2 = f(Some("u2")).await.expect("flags u2").0;
    assert_eq!(u2["groups"], json!(["alpha"]));
    assert_eq!(u2["overrides"]["f1"]["state"], json!("off"));
    let anon = f(None).await.expect("flags anon").0;
    assert_eq!(anon["groups"], json!([]));
    assert_eq!(anon["group_targeted"], json!([]));
    assert_eq!(anon["overrides"], json!({}));
    let resolved = groups::resolve(State(st.clone()), q(json!({ "user": "u2" })))
        .await
        .expect("resolve")
        .0;
    assert_eq!(resolved["groups"][0]["name"], json!("alpha"));
    assert_eq!(resolved["groups"][0]["via"], json!("member"));
    assert_eq!(
        groups::resolve(State(st.clone()), q(json!({})))
            .await
            .err()
            .map(|e| e.0),
        Some(StatusCode::BAD_REQUEST)
    );

    let get = |key: &str, user: Option<&str>| {
        dashboard::experiments_get(State(st.clone()), q(json!({ "key": key, "user": user })))
    };
    assert_eq!(
        get("e1", Some("u1")).await.expect("e1 u1").0,
        json!({ "killed": true, "variant": null, "flags": {} })
    );
    assert_eq!(
        get("e1", Some("u2")).await.expect("e1 u2").0,
        json!({ "killed": false, "variant": "va", "flags": {} })
    );
    assert_eq!(get("e1", None).await.expect("e1 anon").0, json!({}));
    let _ = dashboard::experiment_set(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "exp_key": "e1", "killed": true, "variant": "x", "flags": { "a": 1 } })),
    )
    .await
    .expect("experiment_set");
    let override_row = json!({ "killed": true, "variant": "x", "flags": { "a": 1 } });
    assert_eq!(get("e1", None).await.expect("e1 override").0, override_row);
    assert_eq!(get("e1", Some("u3")).await.expect("e1 u3").0, override_row);
    assert_eq!(
        get("e1", Some("u2")).await.expect("e1 u2 again").0,
        json!({ "killed": false, "variant": "va", "flags": {} })
    );
    let all = dashboard::experiments_get(State(st.clone()), q(json!({})))
        .await
        .expect("all overrides")
        .0;
    assert_eq!(all["e1"], override_row);
    let _ = dashboard::experiment_set(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "exp_key": "e1", "clear": true })),
    )
    .await
    .expect("experiment clear");
    assert_eq!(get("e1", None).await.expect("e1 cleared").0, json!({}));
    assert_eq!(
        audits(pool, "experiment.set").await[0]["flags"],
        json!({ "a": 1 })
    );
    assert_eq!(audits(pool, "experiment.clear").await.len(), 1);

    let _ = groups::set_target(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "group": "alpha", "flag": "f2", "clear": true })),
    )
    .await
    .expect("target clear");
    let _ = groups::set_area(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "kind": "flag", "name": "f1", "clear": true })),
    )
    .await
    .expect("area clear");
    let _ = groups::set(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "name": "beta", "clear": true })),
    )
    .await
    .expect("group clear");
    let list = groups::list(State(st.clone())).await.expect("list2").0;
    assert_eq!(list["groups"].as_array().unwrap().len(), 1);
    assert_eq!(list["groups"][0]["name"], json!("alpha"));
    assert_eq!(
        list["groups"][0]["flag_targets"].as_array().unwrap().len(),
        1
    );
    assert_eq!(list["areas"], json!([]));
    assert_eq!(audits(pool, "group.clear").await.len(), 1);
    assert_eq!(audits(pool, "group.target.clear").await.len(), 1);
    assert_eq!(audits(pool, "area.clear").await.len(), 1);
    let u1 = f(Some("u1")).await.expect("flags u1 after").0;
    assert_eq!(u1["groups"], json!(["alpha"]));
    assert_eq!(u1["overrides"]["f1"]["state"], json!("off"));
    let (targets_left,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM telemetry.flag_group_targets WHERE group_name = 'beta'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(targets_left, 0, "deleting a group cascades to its targets");

    let issue = dashboard::set_issue_state(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "fingerprint": "fp-1", "status": "resolved", "assignee": "alice" })),
    )
    .await
    .expect("issue state")
    .0;
    assert_eq!(
        issue,
        json!({ "ok": true, "fingerprint": "fp-1", "status": "resolved", "assignee": "alice" })
    );
    let kept = dashboard::set_issue_state(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "fingerprint": "fp-1", "note": "n" })),
    )
    .await
    .expect("issue note")
    .0;
    assert_eq!(
        kept["assignee"],
        json!("alice"),
        "an absent assignee keeps the row's"
    );
    let cleared = dashboard::set_issue_state(
        admin_of(&st).await,
        State(st.clone()),
        body(json!({ "fingerprint": "fp-1", "assignee": null })),
    )
    .await
    .expect("issue clear")
    .0;
    assert_eq!(
        cleared["assignee"],
        Value::Null,
        "an explicit null clears it"
    );
    assert_eq!(
        dashboard::set_issue_state(
            admin_of(&st).await,
            State(st.clone()),
            body(json!({ "fingerprint": "fp-1", "status": "nope" }))
        )
        .await
        .err()
        .map(|e| e.0),
        Some(StatusCode::BAD_REQUEST)
    );
    assert_eq!(audits(pool, "issue.state").await.len(), 3);

    let sql = dashboard::sql_query(
        State(st.clone()),
        body(json!({ "sql": "SELECT count(*) AS n FROM telemetry.flag_groups;" })),
    )
    .await
    .expect("sql")
    .0;
    assert_eq!(sql["rows"][0]["n"], json!(1));
    let bad = dashboard::sql_query(
        State(st.clone()),
        body(json!({ "sql": "SELECT * FROM telemetry.nope" })),
    )
    .await;
    assert!(bad.is_err());
    let again = dashboard::sql_query(
        State(st.clone()),
        body(json!({ "sql": "SELECT 1 AS one, current_setting('statement_timeout') AS timeout, current_setting('transaction_read_only') AS ro" })),
    )
    .await
    .expect("sql after error")
    .0;
    assert_eq!(again["rows"][0]["one"], json!(1), "{again}");
    assert_eq!(again["rows"][0]["timeout"], json!("15s"));
    assert_eq!(again["rows"][0]["ro"], json!("on"));
    let write = dashboard::sql_query(
        State(st.clone()),
        body(json!({ "sql": "WITH x AS (INSERT INTO telemetry.flag_groups (name) VALUES ('zz') RETURNING 1) SELECT * FROM x" })),
    )
    .await;
    assert_eq!(write.err().map(|e| e.0), Some(StatusCode::BAD_REQUEST));
    assert_eq!(
        dashboard::sql_query(
            State(st.clone()),
            body(json!({ "sql": "DELETE FROM telemetry.flag_groups" }))
        )
        .await
        .err()
        .map(|e| e.0),
        Some(StatusCode::BAD_REQUEST)
    );
}

#[tokio::test]
async fn admin_writes_audit_in_one_statement() {
    let Some(s) = scratch(settings(Some("tok"))).await else {
        return;
    };
    let st = s.state.clone();
    let pool = &s.pool;
    let aq = || q::<admin::ActorQuery>(json!({}));
    let day = 24 * 3600;

    seed(pool, "segment", "track", 10 * day, json!({ "e": 1 })).await;
    seed(pool, "segment", "track", 9 * day, json!({ "e": 2 })).await;
    seed(pool, "segment", "track", day, json!({ "e": 3 })).await;
    let s1 = seed(
        pool,
        "sentry",
        "event",
        3 * day,
        json!({ "level": "error" }),
    )
    .await;
    let s2 = seed(pool, "sentry", "event", 2 * day, json!({ "level": "info" })).await;
    let s3 = seed(pool, "sentry", "event", day, json!({ "level": "error" })).await;

    let purged = admin::purge(
        admin_of(&st).await,
        State(st.clone()),
        actor("alice"),
        aq(),
        body(json!({ "older_than_days": 5, "source": "segment" })),
    )
    .await
    .expect("purge")
    .0;
    assert_eq!(
        purged,
        json!({ "ok": true, "deleted": 2, "batches": 1, "exhausted": false })
    );
    let purge_audit = audits(pool, "purge").await;
    assert_eq!(purge_audit[0]["deleted"], json!(2));
    assert_eq!(
        (
            purge_audit[0]["batches"].as_i64(),
            purge_audit[0]["exhausted"].as_bool()
        ),
        (Some(1), Some(false))
    );
    assert_eq!(purge_audit[0]["source"], json!("segment"));
    assert_eq!(
        purge_audit[0]["operator"],
        json!("alice"),
        "the console label rides in the detail"
    );
    let (actor_name,): (String,) =
        sqlx::query_as("SELECT actor FROM ops.admin_audit WHERE action = 'purge'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        actor_name, "token:CATALYRST_TELEMETRY_ADMIN_TOKEN",
        "the actor is the verified credential"
    );

    let exported = admin::export(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({ "source": "sentry", "limit": 2 })),
    )
    .await
    .expect("export")
    .0;
    assert_eq!(exported["count"], json!(2));
    assert_eq!(exported["truncated"], json!(true));
    let ids: Vec<i64> = exported["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, vec![s3, s2]);
    let mut keys: Vec<&str> = exported["events"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "body",
            "event_kind",
            "fingerprint",
            "id",
            "project",
            "received_at",
            "source"
        ]
    );
    assert!(
        exported["events"][0]["received_at"]
            .as_str()
            .unwrap()
            .ends_with('Z')
    );
    let export_audit = audits(pool, "export").await;
    assert_eq!(export_audit[0]["count"], json!(2));
    assert_eq!(export_audit[0]["level"], Value::Null);
    assert_eq!(export_audit[0]["operator"], json!("loopback"));

    let deleted = admin::bulk_delete(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({ "level": "error" })),
    )
    .await
    .expect("bulk_delete")
    .0;
    assert_eq!(deleted["deleted"], json!(2));
    assert_eq!(deleted["batches"], json!(1));
    assert_eq!(audits(pool, "bulk_delete").await[0]["deleted"], json!(2));
    let remaining: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM telemetry.events WHERE source = 'sentry' ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(remaining, vec![s2]);
    assert!(s1 != s2);
    let unfiltered = admin::bulk_delete(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({})),
    )
    .await;
    assert_eq!(unfiltered.err().map(|e| e.0), Some(StatusCode::BAD_REQUEST));
    let before = chrono::Utc::now().naive_utc() - chrono::Duration::hours(36);
    let by_before = admin::bulk_delete(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({ "before": before.format("%Y-%m-%dT%H:%M:%SZ").to_string() })),
    )
    .await
    .expect("bulk before")
    .0;
    assert_eq!(
        by_before["deleted"],
        json!(1),
        "the two-day-old sentry row goes, the one-day-old segment row stays"
    );

    let _ = admin::ingest_toggle(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({ "enabled": false })),
    )
    .await
    .expect("toggle off");
    assert!(!st.ingest.enabled.load(std::sync::atomic::Ordering::Relaxed));
    let (v,): (bool,) =
        sqlx::query_as("SELECT enabled FROM telemetry.ingest_settings WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(!v);
    let out = sentry::store(
        State(st.clone()),
        Path("p".into()),
        HeaderMap::new(),
        bytes(&json!({ "event_id": "dropped" })),
    )
    .await;
    assert_eq!(
        out.0,
        json!({ "id": "" }),
        "the kill switch answers success and stores nothing"
    );
    let _ = admin::ingest_toggle(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({ "enabled": true })),
    )
    .await
    .expect("toggle on");
    assert!(st.ingest.enabled.load(std::sync::atomic::Ordering::Relaxed));
    assert_eq!(audits(pool, "ingest_toggle").await.len(), 2);

    let _ = admin::quota(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({ "project": "p", "daily_limit": 5 })),
    )
    .await
    .expect("quota set");
    assert_eq!(st.ingest.quotas.read().unwrap().get("p"), Some(&5));
    let (limit,): (i64,) =
        sqlx::query_as("SELECT daily_limit FROM telemetry.project_quotas WHERE project = 'p'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(limit, 5);
    assert_eq!(
        admin::quota(
            admin_of(&st).await,
            State(st.clone()),
            HeaderMap::new(),
            aq(),
            body(json!({ "project": "p", "daily_limit": -1 }))
        )
        .await
        .err()
        .map(|e| e.0),
        Some(StatusCode::BAD_REQUEST)
    );
    let _ = admin::quota(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({ "project": "p" })),
    )
    .await
    .expect("quota clear");
    assert!(st.ingest.quotas.read().unwrap().get("p").is_none());
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM telemetry.project_quotas")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
    let quota_audit = audits(pool, "quota").await;
    assert_eq!(quota_audit.len(), 2);
    assert_eq!(quota_audit[1]["daily_limit"], Value::Null);

    let regrouped = admin::regroup(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({ "canonical": "c", "sources": ["a", " b ", "a", "c", ""] })),
    )
    .await
    .expect("regroup")
    .0;
    assert_eq!(
        regrouped,
        json!({ "ok": true, "canonical": "c", "merged": 3 })
    );
    let merges: Vec<(String, String)> = sqlx::query_as(
        "SELECT source_fingerprint, canonical_fingerprint FROM telemetry.issue_merges ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(
        merges,
        vec![("a".into(), "c".into()), ("b".into(), "c".into())]
    );
    let _ = admin::regroup(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({ "canonical": "d", "sources": ["a"] })),
    )
    .await
    .expect("regroup again");
    let (canon,): (String,) = sqlx::query_as(
        "SELECT canonical_fingerprint FROM telemetry.issue_merges WHERE source_fingerprint = 'a'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(canon, "d");
    let regroup_audit = audits(pool, "regroup").await;
    assert_eq!(regroup_audit[0]["sources"], json!(["a", "b", "a"]));
    assert_eq!(regroup_audit[0]["merged"], json!(3));
    assert_eq!(regroup_audit[0]["fingerprint"], json!("c"));
    let listed = admin::audit_list(
        admin_of(&st).await,
        State(st.clone()),
        q(json!({ "fingerprint": "c" })),
    )
    .await
    .expect("audit_list")
    .0;
    assert_eq!(listed["count"], json!(1));
    assert_eq!(listed["items"][0]["action"], json!("regroup"));
    assert_eq!(listed["items"][0]["target"], json!("c"));
    let by_action = admin::audit_list(
        admin_of(&st).await,
        State(st.clone()),
        q(json!({ "action": "quota" })),
    )
    .await
    .expect("audit by action")
    .0;
    assert_eq!(by_action["count"], json!(2));

    let released = admin::release(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        q(json!({ "actor": "bob" })),
        body(json!({ "release": "r1", "state": "broken", "note": "" })),
    )
    .await
    .expect("release")
    .0;
    assert_eq!(
        released,
        json!({ "ok": true, "release": "r1", "state": "broken" })
    );
    let (state_v, note): (String, Option<String>) =
        sqlx::query_as("SELECT state, note FROM telemetry.release_state WHERE release = 'r1'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!((state_v.as_str(), note), ("broken", None));
    assert_eq!(audits(pool, "release").await[0]["operator"], json!("bob"));
    assert_eq!(
        admin::release(
            admin_of(&st).await,
            State(st.clone()),
            HeaderMap::new(),
            aq(),
            body(json!({ "release": "r1", "state": "nope" }))
        )
        .await
        .err()
        .map(|e| e.0),
        Some(StatusCode::BAD_REQUEST)
    );
}

#[tokio::test]
async fn the_purge_job_deletes_in_bounded_batches() {
    let Some(s) = scratch(settings(None)).await else {
        return;
    };
    let pool = &s.pool;
    let day = 24 * 3600;
    let old = seed(
        pool,
        "sentry",
        "event",
        100 * day,
        json!({ "message": "old" }),
    )
    .await;
    let stale_error = seed(
        pool,
        "segment",
        "track",
        40 * day,
        json!({ "event": "client_error" }),
    )
    .await;
    let fresh_error = seed(
        pool,
        "segment",
        "track",
        10 * day,
        json!({ "event": "client_error" }),
    )
    .await;
    let recent = seed(pool, "sentry", "event", day, json!({ "message": "recent" })).await;
    let kept_track = seed(
        pool,
        "segment",
        "track",
        40 * day,
        json!({ "event": "other" }),
    )
    .await;

    let now = chrono::Utc::now().naive_utc();
    let budget = purge::purge_bounded(pool, now, 90, 1, None).await.unwrap();
    assert_eq!(
        budget,
        purge::PassReport {
            expired: 1,
            client_errors: 0,
            batches: 1,
            exhausted: true
        },
        "one batch, then the budget"
    );
    let report = purge::purge_bounded(pool, now, 90, 100, None)
        .await
        .unwrap();
    assert_eq!(
        report,
        purge::PassReport {
            expired: 0,
            client_errors: 1,
            batches: 2,
            exhausted: false
        }
    );
    let left: Vec<i64> = sqlx::query_scalar("SELECT id FROM telemetry.events ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap();
    assert_eq!(left, vec![fresh_error, recent, kept_track]);
    assert!(old != stale_error);
    let again = purge::purge_bounded(pool, now, 90, 100, None)
        .await
        .unwrap();
    assert_eq!(
        again,
        purge::PassReport {
            expired: 0,
            client_errors: 0,
            batches: 2,
            exhausted: false
        }
    );

    // The operator purge takes the same batches under the same budget: with
    // one batch allowed and a batch smaller than the backlog, it reports
    // exhausted and the next call continues where it stopped.
    let st = Arc::new(
        TelemetryState::new(
            pool.clone(),
            Outbound::for_tests(),
            Settings {
                purge_max_batches: 1,
                ..settings(Some("tok"))
            },
        )
        .await
        .unwrap(),
    );
    sqlx::query("INSERT INTO telemetry.events (source, project, event_kind, received_at, body) SELECT 'sentry', 'bulk', 'event', timezone('UTC', now()) - interval '200 days', '{}'::jsonb FROM generate_series(1, 6000)")
        .execute(pool)
        .await
        .unwrap();
    let first = admin::purge(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        q(json!({})),
        body(json!({ "older_than_days": 100, "project": "bulk" })),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(
        first,
        json!({ "ok": true, "deleted": 5000, "batches": 1, "exhausted": true })
    );
    let second = admin::purge(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        q(json!({})),
        body(json!({ "older_than_days": 100, "project": "bulk" })),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(
        second,
        json!({ "ok": true, "deleted": 1000, "batches": 1, "exhausted": false })
    );
    let audit = audits(pool, "purge").await;
    assert_eq!(audit.len(), 2);
    assert_eq!(
        (
            audit[0]["exhausted"].as_bool(),
            audit[1]["exhausted"].as_bool()
        ),
        (Some(true), Some(false))
    );
}

#[tokio::test]
async fn the_router_gates_the_sinks_the_console_and_the_login() {
    let Some(s) = scratch(settings(Some("tok"))).await else {
        return;
    };
    let app = router(s.state.clone());
    let track = json!({ "writeKey": "wk", "type": "track", "event": "hi" }).to_string();

    // Both switches on: the sink stores, the console answers, the bearer gates the writes.
    let (status, _, body) = send(
        &app,
        "POST",
        "/v1/track",
        &[("content-type", "application/json")],
        Some(&track),
    )
    .await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, "{\"success\":true}")
    );
    let (status, _, _) = send(
        &app,
        "POST",
        "/api/42/envelope/",
        &[],
        Some("{\"event_id\":\"x\"}\n"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the trailing-slash form answers too"
    );
    let (status, _, body) = send(&app, "GET", "/dash/events?hours=1", &[], None).await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::FORBIDDEN, "invalid admin bearer")
    );
    let (status, headers, _) = send(&app, "GET", "/", &[("accept", "text/html")], None).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        headers.get(header::LOCATION).unwrap(),
        "/login",
        "an HTML request is sent to the login form"
    );
    let (status, _, _) = send(&app, "GET", "/login", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, headers, _) = send(
        &app,
        "POST",
        "/login",
        &[("content-type", "application/x-www-form-urlencoded")],
        Some("token=tok"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let cookie = headers
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        cookie.starts_with("telemetry_admin=")
            && cookie.contains("; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=2592000"),
        "{cookie}"
    );
    assert!(
        !cookie.contains("=tok;"),
        "the session cookie never carries the bearer: {cookie}"
    );
    let session = cookie
        .trim_start_matches("telemetry_admin=")
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let session_header = format!("telemetry_admin={session}");
    assert_eq!(headers.get(header::LOCATION).unwrap(), "/");
    let (status, headers, _) = send(
        &app,
        "POST",
        "/login",
        &[("content-type", "application/x-www-form-urlencoded")],
        Some("token=wrong"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers.get(header::LOCATION).unwrap(), "/login?err=1");
    let (status, _, _) = send(
        &app,
        "GET",
        "/dash/events?hours=1",
        &[("cookie", "telemetry_admin=tok")],
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the raw token pasted as the cookie is not a session"
    );
    let (status, _, _) = send(
        &app,
        "GET",
        "/dash/events?hours=1",
        &[("cookie", session_header.as_str())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the session opens the reads");
    let (status, _, body) = send(
        &app,
        "GET",
        "/",
        &[("cookie", session_header.as_str()), ("accept", "text/html")],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("window.__BASE__=\"\"") && !body.contains("<!--SSR:rows-->"),
        "the page renders with its markers consumed"
    );
    let (status, _, _) = send(&app, "GET", "/flags", &[], None).await;
    assert_eq!(status, StatusCode::OK, "the public pages need no session");
    let (status, _, _) = send(&app, "GET", "/fonts/archivo-400.woff2", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    let flag = json!({ "flag": "f", "state": "on" }).to_string();
    let (status, _, _) = send(
        &app,
        "POST",
        "/dash/flag",
        &[
            ("cookie", session_header.as_str()),
            ("content-type", "application/json"),
        ],
        Some(&flag),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a write needs the bearer, not the cookie"
    );
    let (status, _, _) = send(
        &app,
        "POST",
        "/dash/flag",
        &[
            ("authorization", "Bearer tok"),
            ("content-type", "application/json"),
        ],
        Some(&flag),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = send(
        &app,
        "POST",
        "/dash/admin/purge",
        &[
            ("authorization", "Bearer nope"),
            ("content-type", "application/json"),
        ],
        Some("{\"older_than_days\":1}"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, body) = send(&app, "GET", "/dash/experiments", &[], None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the experiments aggregate is public, as before"
    );
    assert!(body.contains("\"experiments\""));

    // Ingest off, writes off: the sinks and the console mutations answer 503, the reads and the login stay open.
    let off = Settings {
        ingest_enabled: false,
        writes_enabled: false,
        ..settings(Some("tok"))
    };
    let mut state = TelemetryState::new(s.pool.clone(), Outbound::for_tests(), off)
        .await
        .unwrap();
    state.sql_pool = reader_pool().await;
    let app = router(Arc::new(state));
    for path in [
        "/v1/track",
        "/v1/batch",
        "/api/42/envelope",
        "/api/42/store",
    ] {
        let (status, headers, body) = send(
            &app,
            "POST",
            path,
            &[("content-type", "application/json")],
            Some(&track),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "3600");
        assert!(body.contains("TELEMETRY_INGEST_ENABLED=false"), "{body}");
    }
    for (path, payload) in [
        ("/dash/flag", flag.as_str()),
        ("/dash/admin/purge", "{\"older_than_days\":1}"),
        ("/dash/issue/state", "{\"fingerprint\":\"x\"}"),
    ] {
        let (status, _, body) = send(
            &app,
            "POST",
            path,
            &[
                ("authorization", "Bearer tok"),
                ("content-type", "application/json"),
            ],
            Some(payload),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert!(body.contains("OPS_WRITES_ENABLED=false"), "{body}");
    }
    let (status, _, _) = send(
        &app,
        "GET",
        "/dash/events?hours=1",
        &[("authorization", "Bearer tok")],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = send(
        &app,
        "POST",
        "/login",
        &[("content-type", "application/x-www-form-urlencoded")],
        Some("token=tok"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "the session mint is not a write"
    );
    let (status, _, _) = send(
        &app,
        "POST",
        "/dash/sql",
        &[
            ("authorization", "Bearer tok"),
            ("content-type", "application/json"),
        ],
        Some("{\"sql\":\"SELECT 1 AS one\"}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the read-only query is not a write");
    let (status, _, body) = send(
        &app,
        "POST",
        "/dash/sql",
        &[
            ("authorization", "Bearer tok"),
            ("content-type", "application/json"),
        ],
        Some("{\"sql\":\"SELECT count(*) AS n FROM credits.wallets\"}"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body.contains("permission denied"),
        "the reader login sees telemetry alone: {body}"
    );

    // No admin token: every gated route fails closed with the old 403.
    let none = Settings {
        admin_token: None,
        ..settings(None)
    };
    let state = Arc::new(
        TelemetryState::new(s.pool.clone(), Outbound::for_tests(), none)
            .await
            .unwrap(),
    );
    let app = router(state);
    let (status, _, body) = send(
        &app,
        "GET",
        "/dash/events?hours=1",
        &[("authorization", "Bearer tok")],
        None,
    )
    .await;
    assert_eq!(
        (status, body.as_str()),
        (
            StatusCode::FORBIDDEN,
            "admin disabled (CATALYRST_TELEMETRY_ADMIN_TOKEN unset)"
        )
    );
    let (status, _, _) = send(
        &app,
        "POST",
        "/dash/admin/regroup",
        &[
            ("authorization", "Bearer tok"),
            ("content-type", "application/json"),
        ],
        Some("{\"canonical\":\"c\",\"sources\":[\"a\"]}"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Functions an administrator might leave in `telemetry` (EXECUTE is
/// PUBLIC's by default): one resets the role before reading credits, one
/// sets it to a named role, one does that inside a DO block, one writes
/// from a DO block. On the reader login each has no wider role to reach.
const PROBES_SQL: &str = "\
    CREATE OR REPLACE FUNCTION telemetry.tsql_reset_role() RETURNS bigint LANGUAGE plpgsql AS $f$ \
      BEGIN RESET ROLE; RETURN (SELECT count(*) FROM credits.packs); END $f$; \
    CREATE OR REPLACE FUNCTION telemetry.tsql_set_role(r text) RETURNS bigint LANGUAGE plpgsql AS $f$ \
      BEGIN EXECUTE format('SET ROLE %I', r); RETURN (SELECT count(*) FROM credits.packs); END $f$; \
    CREATE OR REPLACE FUNCTION telemetry.tsql_do_set_role(r text) RETURNS bigint LANGUAGE plpgsql AS $f$ \
      BEGIN EXECUTE format('DO $d$ BEGIN SET ROLE %I; END $d$', r); RETURN (SELECT count(*) FROM credits.packs); END $f$; \
    CREATE OR REPLACE FUNCTION telemetry.tsql_do_write() RETURNS int LANGUAGE plpgsql AS $f$ \
      BEGIN EXECUTE 'DO $d$ BEGIN CREATE TABLE telemetry.tsql_evil (); END $d$'; RETURN 1; END $f$";

/// `POST /dash/sql` runs on the reader's own login: telemetry's tables
/// answer (qualified, through the search path, in a CTE) as the reader,
/// read-only, under the 15 s timeout; every escape this recheck exercises
/// and its relatives is refused by the database and returns no row:
/// the role switched back (`set_config('role', 'none')`, `RESET ROLE` in a
/// function) or forward (`set_config`, a function, a DO block run by a
/// function) to the runtime login, the session authorization, reads of the
/// credits, social, economy, worlds and audit schemas, `pg_authid`,
/// `pg_shadow`, the server files and large objects, `dblink` (absent), and
/// every write; what a statement leaves on its session (a search path, an
/// advisory lock) ends with the request.
#[tokio::test]
async fn dash_sql_runs_on_the_reader_login_and_refuses_every_escape() {
    let Some(s) = scratch(settings(Some("tok"))).await else {
        return;
    };
    let st = s.state.clone();
    let runtime: String = sqlx::query_scalar("SELECT current_user::text")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    sqlx::raw_sql(PROBES_SQL)
        .execute(&s.pool)
        .await
        .expect("the probe functions");
    seed(&s.pool, "sentry", "event", 1, json!({ "message": "one" })).await;
    let packs: i64 = sqlx::query_scalar("SELECT count(*) FROM credits.packs")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert!(
        packs > 0,
        "schema.sql seeds the credit packs the escape used to read"
    );
    let run = |sql: String| {
        let st = st.clone();
        async move { dashboard::sql_query(State(st), body(json!({ "sql": sql }))).await }
    };

    let who = run("select current_user::text as u, session_user::text as s, current_setting('statement_timeout') as t, \
                   current_setting('transaction_read_only') as ro, current_setting('search_path') as p".into())
        .await
        .expect("who")
        .0;
    assert_eq!(
        who["rows"][0],
        json!({ "u": READER, "s": READER, "t": "15s", "ro": "on", "p": "telemetry, pg_catalog" }),
        "a column named t is a column, not the row"
    );
    let ok = run("select count(*) as n from telemetry.events".into())
        .await
        .expect("telemetry qualified")
        .0;
    assert_eq!(ok["rows"][0]["n"], json!(1));
    let ok = run("select count(*) as n from events".into())
        .await
        .expect("telemetry through the search_path")
        .0;
    assert_eq!(ok["rows"][0]["n"], json!(1));
    let ok = run("with w as (select 1 as one) select one from w".into())
        .await
        .expect("a CTE")
        .0;
    assert_eq!(ok["rows"][0]["one"], json!(1));

    let refused: Vec<(String, &str)> = vec![
        ("select set_config('role', 'none', true) as r, query_to_xml('select string_agg(sku, chr(44)) as skus from credits.packs', true, false, '')::text as x".into(), "permission denied for schema credits"),
        ("select set_config('role', 'none', true) as r, query_to_xml('select count(*) as n from ops.admin_audit', true, false, '')::text as x".into(), "permission denied for schema ops"),
        (format!("select set_config('role', '{runtime}', true) as r"), "permission denied to set role"),
        (format!("select set_config('role', '{runtime}', true) as r, (select count(*) from credits.ledger) as n"), "permission denied"),
        (format!("select set_config('session_authorization', '{runtime}', true) as r"), "permission denied to set session authorization"),
        ("select telemetry.tsql_reset_role() as n".into(), "permission denied for schema credits"),
        (format!("select telemetry.tsql_set_role('{runtime}') as n"), "permission denied to set role"),
        (format!("select telemetry.tsql_do_set_role('{runtime}') as n"), "permission denied to set role"),
        ("select telemetry.tsql_do_write() as n".into(), "read-only transaction"),
        ("select * from credits.packs".into(), "permission denied for schema credits"),
        ("select count(*) as n from credits.ledger".into(), "permission denied for schema credits"),
        ("select count(*) as n from credits.wallets".into(), "permission denied for schema credits"),
        ("select count(*) as n from social.friend_messages".into(), "permission denied for schema social"),
        ("select count(*) as n from economy.transactions".into(), "permission denied for schema economy"),
        ("select count(*) as n from worlds.env_variables".into(), "permission denied for schema worlds"),
        ("select count(*) as n from content.deployments".into(), "permission denied for schema content"),
        ("select actor from ops.admin_audit".into(), "permission denied for schema ops"),
        ("select rolname, rolpassword from pg_authid".into(), "permission denied for table pg_authid"),
        ("select usename, passwd from pg_shadow".into(), "permission denied for view pg_shadow"),
        ("select dblink('dbname=postgres', 'select 1') as d".into(), "does not exist"),
        ("select pg_read_file('/etc/hostname') as f".into(), "permission denied for function pg_read_file"),
        ("select pg_read_binary_file('/etc/hostname') as f".into(), "permission denied for function pg_read_binary_file"),
        ("select pg_ls_dir('.') as f".into(), "permission denied for function pg_ls_dir"),
        ("select lo_import('/etc/hostname') as o".into(), "permission denied for function lo_import"),
        ("select set_config('transaction_read_only', 'off', true) as r".into(), "read-write mode must be set before any query"),
        ("with x as (insert into telemetry.flag_groups (name) values ('zz') returning 1) select * from x".into(), ""),
        ("select query_to_xml('insert into telemetry.flag_groups (name) values (''zz'') returning name', true, false, '')::text as x".into(), ""),
    ];
    for (sql, expected) in &refused {
        match run(sql.clone()).await {
            Err((status, message)) => {
                assert_eq!(status, StatusCode::BAD_REQUEST, "{sql}: {message}");
                assert!(
                    message.contains(expected),
                    "{sql}: expected {expected:?} in {message}"
                );
            }
            Ok(Json(rows)) => panic!("{sql} answered {rows}"),
        }
    }
    for (sql, why) in [
        (
            "do $$ begin set role postgres; end $$",
            "only SELECT / WITH queries are allowed",
        ),
        (
            "select 1; do $$ begin set role postgres; end $$",
            "one statement only (no ';')",
        ),
        ("reset role", "only SELECT / WITH queries are allowed"),
    ] {
        let (status, message) = run(sql.into()).await.expect_err(sql);
        assert_eq!((status, message.as_str()), (StatusCode::BAD_REQUEST, why));
    }
    let zz: i64 =
        sqlx::query_scalar("SELECT count(*) FROM telemetry.flag_groups WHERE name = 'zz'")
            .fetch_one(&s.pool)
            .await
            .unwrap();
    assert_eq!(zz, 0, "no write landed");

    let moved = run("select set_config('search_path', 'credits', false) as p, set_config('role', 'none', false) as r".into()).await.expect("a session-level set").0;
    assert_eq!(moved["rows"][0]["p"], json!("credits"));
    let back = run("select count(*) as n, current_setting('search_path') as p from events".into())
        .await
        .expect("the next request")
        .0;
    assert_eq!(
        back["rows"][0],
        json!({ "n": 1, "p": "telemetry, pg_catalog" }),
        "the rollback and DISCARD ALL undo it"
    );

    const KEY: i64 = 7_242_711;
    let held = run(format!(
        "select pg_advisory_lock({KEY}) is not null as held"
    ))
    .await
    .expect("an advisory lock")
    .0;
    assert_eq!(held["rows"][0]["held"], json!(true));
    let mut probe = s.pool.acquire().await.unwrap();
    assert!(
        crate::shared::test_wait::until(async || {
            sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1)")
                .bind(KEY)
                .fetch_one(&mut *probe)
                .await
                .unwrap()
        })
        .await,
        "the session-level advisory lock outlived its request"
    );
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(KEY)
        .execute(&mut *probe)
        .await
        .unwrap();

    let after = run("select current_user::text as u, count(*) as n from telemetry.events".into())
        .await
        .expect("still the reader")
        .0;
    assert_eq!(after["rows"][0], json!({ "u": READER, "n": 1 }));
    sqlx::raw_sql("DROP FUNCTION telemetry.tsql_reset_role(), telemetry.tsql_set_role(text), telemetry.tsql_do_set_role(text), telemetry.tsql_do_write()")
        .execute(&s.pool)
        .await
        .unwrap();
}

/// The login itself, outside the route's statement filter: a DO block
/// setting the role, `SET SESSION AUTHORIZATION` and `SET ROLE` are refused
/// (no membership, no superuser session); even in an explicit `READ WRITE`
/// transaction it creates nothing, writes nothing and reads no other
/// schema (the grants are SELECT on telemetry alone); and a timeout armed
/// for the statement is not disarmed by `set_config` inside it.
#[tokio::test]
async fn the_reader_login_itself_holds_nothing_but_telemetry_reads() {
    let Some(s) = scratch(settings(Some("tok"))).await else {
        return;
    };
    let reader = s.state.sql_pool.clone().expect("the reader pool");
    let runtime: String = sqlx::query_scalar("SELECT current_user::text")
        .fetch_one(&s.pool)
        .await
        .unwrap();

    for (begin, statement, expected) in [
        (
            "BEGIN READ ONLY",
            format!("DO $$ BEGIN EXECUTE format('SET ROLE %I', '{runtime}'); END $$"),
            "permission denied to set role",
        ),
        (
            "BEGIN READ ONLY",
            format!("DO $$ BEGIN PERFORM set_config('role', '{runtime}', false); END $$"),
            "permission denied to set role",
        ),
        (
            "BEGIN READ ONLY",
            format!("SET ROLE \"{runtime}\""),
            "permission denied to set role",
        ),
        (
            "BEGIN READ ONLY",
            format!("SET SESSION AUTHORIZATION \"{runtime}\""),
            "permission denied to set session authorization",
        ),
        (
            "BEGIN READ ONLY",
            "DO $$ BEGIN CREATE TABLE telemetry.tsql_evil (); END $$".to_string(),
            "read-only transaction",
        ),
        (
            "BEGIN READ ONLY",
            "CREATE TEMP TABLE tsql_evil (x int)".to_string(),
            "read-only transaction",
        ),
        (
            "BEGIN READ WRITE",
            "CREATE TABLE telemetry.tsql_evil ()".to_string(),
            "permission denied for schema telemetry",
        ),
        (
            "BEGIN READ WRITE",
            "CREATE SCHEMA tsql_evil".to_string(),
            "permission denied for database",
        ),
        (
            "BEGIN READ WRITE",
            "INSERT INTO telemetry.flag_groups (name) VALUES ('zz')".to_string(),
            "permission denied for table flag_groups",
        ),
        (
            "BEGIN READ WRITE",
            "DELETE FROM telemetry.events".to_string(),
            "permission denied for table events",
        ),
        (
            "BEGIN READ WRITE",
            "SELECT count(*) FROM credits.packs".to_string(),
            "permission denied for schema credits",
        ),
        (
            "BEGIN READ WRITE",
            "RESET ROLE; SELECT count(*) FROM credits.packs".to_string(),
            "permission denied for schema credits",
        ),
    ] {
        let mut tx = reader
            .begin_with(begin)
            .await
            .expect("a transaction on the reader login");
        let err = sqlx::raw_sql(sqlx::AssertSqlSafe(statement.clone()))
            .execute(&mut *tx)
            .await
            .expect_err(&statement);
        assert!(
            err.to_string().contains(expected),
            "{begin}; {statement}: expected {expected:?} in {err}"
        );
        let _ = tx.rollback().await;
    }

    // The statement waits on a lock another session holds, so the armed timeout is its
    // only way out; a disarmed one would leave it waiting into the hang guard.
    const HELD: i64 = 7_242_712;
    let mut holder = s.pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(HELD)
        .execute(&mut *holder)
        .await
        .unwrap();
    let mut tx = reader
        .begin_with("BEGIN READ ONLY; SET LOCAL statement_timeout = 300")
        .await
        .unwrap();
    let err = tokio::time::timeout(
        crate::shared::test_wait::HANG_GUARD,
        sqlx::query("SELECT set_config('statement_timeout', '0', true), pg_advisory_lock($1)")
            .bind(HELD)
            .execute(&mut *tx),
    )
    .await
    .expect("set_config did not disarm the running statement's timeout")
    .expect_err("the armed timeout");
    assert!(err.to_string().contains("statement timeout"), "{err}");
    let _ = tx.rollback().await;
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(HELD)
        .execute(&mut *holder)
        .await
        .unwrap();
}

/// The route checks the login's posture before every statement: a DSN
/// that logs in as a wider role (the scratch pool's own superuser, as a
/// runtime DSN pasted into the variable would) or a reader granted one
/// schema too many answers 503 listing the excess and runs nothing; an
/// unset variable answers 503 naming it (never the runtime pool); a login
/// the database refuses answers 503 without the DSN.
#[tokio::test]
async fn dash_sql_refuses_a_login_wider_than_telemetry_and_an_absent_one() {
    let Some(s) = scratch(settings(Some("tok"))).await else {
        return;
    };
    let query = || body(json!({ "sql": "select count(*) as n from telemetry.events" }));

    let mut wide =
        TelemetryState::new(s.pool.clone(), Outbound::for_tests(), settings(Some("tok")))
            .await
            .unwrap();
    wide.sql_pool = Some(s.pool.clone());
    let (status, message) = dashboard::sql_query(State(Arc::new(wide)), query())
        .await
        .expect_err("a superuser login");
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    for excess in [
        "SUPERUSER",
        "USAGE on schema credits",
        "USAGE on schema social",
        "TELEMETRY_SQL_DATABASE_URL",
    ] {
        assert!(message.contains(excess), "{excess} in {message}");
    }

    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "GRANT USAGE ON SCHEMA credits TO {READER}"
    )))
    .execute(&s.pool)
    .await
    .unwrap();
    let granted = dashboard::sql_query(State(s.state.clone()), query()).await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "REVOKE USAGE ON SCHEMA credits FROM {READER}"
    )))
    .execute(&s.pool)
    .await
    .unwrap();
    let (status, message) = granted.expect_err("one schema too many");
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        message.contains("USAGE on schema credits") && !message.contains("SUPERUSER"),
        "{message}"
    );
    let ok = dashboard::sql_query(State(s.state.clone()), query())
        .await
        .expect("revoked again")
        .0;
    assert_eq!(ok["rows"][0]["n"], json!(0));

    // the two built-ins that write from a READ ONLY transaction: refused to the
    // reader, and a reader granted one of them is a posture the route refuses
    for forged in [
        "select pg_notify('new_deployment', 'scene:forged') as n",
        "select pg_logical_emit_message(false, 'x', 'y') as n",
    ] {
        let refused =
            dashboard::sql_query(State(s.state.clone()), body(json!({ "sql": forged }))).await;
        let (_, message) = refused.expect_err("no EXECUTE on the built-in");
        assert!(message.contains("permission denied"), "{forged}: {message}");
    }
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "GRANT EXECUTE ON FUNCTION pg_catalog.pg_notify(text, text) TO {READER}"
    )))
    .execute(&s.pool)
    .await
    .unwrap();
    let granted = dashboard::sql_query(State(s.state.clone()), query()).await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "REVOKE EXECUTE ON FUNCTION pg_catalog.pg_notify(text, text) FROM {READER}"
    )))
    .execute(&s.pool)
    .await
    .unwrap();
    let (status, message) = granted.expect_err("pg_notify granted");
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        message.contains("EXECUTE on pg_notify(text,text)"),
        "{message}"
    );

    let unset = Arc::new(
        TelemetryState::new(s.pool.clone(), Outbound::for_tests(), settings(Some("tok")))
            .await
            .unwrap(),
    );
    assert!(
        unset.sql_pool.is_none(),
        "no DSN, no pool: the runtime pool is never the fallback"
    );
    let (status, message) = dashboard::sql_query(State(unset), query())
        .await
        .expect_err("unset");
    assert_eq!(
        (status, message.as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, sql::NOT_CONFIGURED)
    );

    let url = crate::db::scratch::url().await.unwrap();
    let opts: sqlx::postgres::PgConnectOptions = url.parse().unwrap();
    let host = opts
        .get_socket()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| opts.get_host().to_string());
    let missing = format!(
        "postgres:///catalyst_no_such_database_tsql?host={host}&port={}&user={READER}&password=hunter2hunter2",
        opts.get_port()
    );
    let refused = Settings {
        sql_reader: Some(sql::ReaderDsn::parse(&missing).unwrap()),
        ..settings(Some("tok"))
    };
    let refused = Arc::new(
        TelemetryState::new(s.pool.clone(), Outbound::for_tests(), refused)
            .await
            .unwrap(),
    );
    let (status, message) = dashboard::sql_query(State(refused), query())
        .await
        .expect_err("a refused login");
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        message.contains("TELEMETRY_SQL_DATABASE_URL")
            && !message.contains("hunter2")
            && !message.contains("catalyst_no_such"),
        "{message}"
    );
}

/// The web tier's two direct readers of `telemetry.events`
/// (`TELEMETRY_DATABASE_URL`: `sites/src/data/lib/catalyst/admin/
/// operator-metrics.server.ts` and `creator-hub/scene-errors.server.ts`),
/// their statements verbatim, run on the reader login: it is the role
/// they use.
#[tokio::test]
async fn the_web_tier_telemetry_readers_run_on_the_reader_login() {
    let Some(s) = scratch(settings(Some("tok"))).await else {
        return;
    };
    let reader = s.state.sql_pool.clone().expect("the reader pool");
    for event in ["Deploy Started", "Deploy Started", "Deploy Completed"] {
        seed(&s.pool, "segment", "track", 60, json!({ "event": event })).await;
    }
    seed(&s.pool, "segment", "track", 30, json!({ "event": "Scene Error", "properties": {
        "scene_hash": "bafyscene", "scene_base": "10,20", "error_kind": "runtime", "error_message": "boom" } })).await;

    let counts: Vec<(String, i32)> = sqlx::query_as(
        "SELECT body->>'event' AS event, count(*)::int AS n
   FROM telemetry.events
   WHERE source = 'segment' AND body->>'event' = ANY($1::text[])
   GROUP BY 1",
    )
    .bind(vec!["Deploy Started", "Deploy Completed", "Ban Issued"])
    .fetch_all(&reader)
    .await
    .expect("operator-metrics COUNTS_SQL");
    let mut counts = counts;
    counts.sort();
    assert_eq!(
        counts,
        vec![
            ("Deploy Completed".to_string(), 1),
            ("Deploy Started".to_string(), 2)
        ]
    );

    let errors: Vec<(chrono::NaiveDateTime, String, String, String)> = sqlx::query_as(
        "SELECT received_at,
            coalesce(body->'properties'->>'scene_hash', '') AS scene_hash,
            coalesce(body->'properties'->>'error_kind', '') AS error_kind,
            left(coalesce(body->'properties'->>'error_message', ''), 2048) AS error_message
       FROM telemetry.events
      WHERE source = 'segment' AND body->>'event' = 'Scene Error'
        AND body->'properties'->>'scene_hash' = ANY($3::text[])
        AND ($1 = 'genesis' AND body->'properties'->>'scene_base' = $2
          OR $1 = 'world' AND lower(body->'properties'->>'scene_world') = lower($2))
      ORDER BY received_at DESC LIMIT 100",
    )
    .bind("genesis")
    .bind("10,20")
    .bind(vec!["bafyscene"])
    .fetch_all(&reader)
    .await
    .expect("scene-errors query");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        (
            errors[0].1.as_str(),
            errors[0].2.as_str(),
            errors[0].3.as_str()
        ),
        ("bafyscene", "runtime", "boom")
    );
}

/// The DSN carries the reader's password: a malformed value is refused
/// naming the variable only, the parsed settings never print it, and a
/// blank value is unset.
#[test]
fn the_reader_dsn_never_prints() {
    let raw = |v: &str| RawSettings {
        sql_database_url: Some(v.into()),
        base_path: "/t".into(),
        ..Default::default()
    };
    let deployed = "postgres:///catalyst?host=/run/postgresql&port=5432&user=catalyst_telemetry_reader&password=hunter2hunter2";
    let settings = Settings::resolve(raw(deployed), &[]).expect("the deployed form");
    assert!(settings.sql_reader.is_some());
    let printed = format!("{settings:?}");
    assert!(
        !printed.contains("hunter2") && printed.contains("ReaderDsn(<redacted>)"),
        "{printed}"
    );
    let err = Settings::resolve(
        raw("catalyst_telemetry_reader:hunter2hunter2 at the socket"),
        &[],
    )
    .expect_err("not a URL")
    .to_string();
    assert!(
        err.contains("TELEMETRY_SQL_DATABASE_URL") && !err.contains("hunter2"),
        "{err}"
    );
    assert!(
        Settings::resolve(raw("  "), &[])
            .unwrap()
            .sql_reader
            .is_none(),
        "blank is unset"
    );
    assert!(Settings::for_tests().sql_reader.is_none());
}

/// Every `/dash/admin/*` route's handler must name the `TelemetryAdmin`
/// extractor, asserted against this module's own source text: the compiler
/// forces the bearer check *if* a handler names it, but cannot force a new
/// route to declare the argument at all. A convention with a script
/// attached, not a type guarantee.
#[test]
fn every_dash_admin_route_names_the_telemetry_admin_extractor() {
    const ROUTER: &str = include_str!("mod.rs");
    const ADMIN: &str = include_str!("admin.rs");
    const HANDLER_PREFIX: &str = "admin::";

    let mut names = Vec::new();
    let mut cursor = 0;
    while let Some(rel) = ROUTER[cursor..].find("\"/dash/admin/") {
        let path_pos = cursor + rel;
        let handler_rel = ROUTER[path_pos..]
            .find(HANDLER_PREFIX)
            .expect("a /dash/admin/ route must name an admin:: fn");
        let ident_start = path_pos + handler_rel + HANDLER_PREFIX.len();
        let ident: String = ROUTER[ident_start..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        assert!(!ident.is_empty());
        names.push(ident);
        cursor = ident_start;
    }
    assert!(
        names.len() >= 8,
        "expected at least the 8 known /dash/admin/* routes, found {names:?}"
    );
    assert!(ADMIN.contains("pub struct TelemetryAdmin") && ADMIN.contains("fn from_request_parts"));
    for name in &names {
        let needle = format!("fn {name}(");
        let start = ADMIN
            .find(&needle)
            .unwrap_or_else(|| panic!("handler `{name}` is defined in admin.rs"));
        let open = start + needle.len() - 1;
        let mut depth = 0usize;
        let mut end = None;
        for (offset, byte) in ADMIN.as_bytes()[open..].iter().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let signature = &ADMIN[start..=end.expect("a terminated parameter list")];
        assert!(
            signature.contains("TelemetryAdmin"),
            "admin handler `{name}` does not name the TelemetryAdmin extractor:\n{signature}"
        );
    }
}

/// A regroup is read back: the issues aggregate folds the sources into the
/// canonical fingerprint, and a `fingerprint=` filter on the events page and
/// the facet stats matches the folded sources too (review finding
/// database-issue-merges-unread: the table had a writer and no reader).
#[tokio::test]
async fn regroup_folds_the_sources_into_the_canonical_issue() {
    let Some(s) = scratch(settings(Some("tok"))).await else {
        return;
    };
    let st = s.state.clone();
    let pool = &s.pool;
    let aq = || q::<admin::ActorQuery>(json!({}));
    seed(
        pool,
        "sentry",
        "event",
        10,
        json!({ "message": "alpha", "user": { "id": "u1" } }),
    )
    .await;
    seed(
        pool,
        "sentry",
        "event",
        20,
        json!({ "message": "alpha", "user": { "id": "u2" } }),
    )
    .await;
    seed(
        pool,
        "sentry",
        "event",
        30,
        json!({ "message": "beta", "user": { "id": "u1" } }),
    )
    .await;
    seed(
        pool,
        "sentry",
        "event",
        40,
        json!({ "message": "gamma", "user": { "id": "u3" } }),
    )
    .await;
    async fn fingerprint_of(pool: &PgPool, title: &str) -> String {
        let (fp,): (String,) = sqlx::query_as(
            "SELECT fingerprint FROM telemetry.events WHERE body->>'message' = $1 LIMIT 1",
        )
        .bind(title)
        .fetch_one(pool)
        .await
        .unwrap();
        fp
    }
    let (alpha, beta) = (
        fingerprint_of(pool, "alpha").await,
        fingerprint_of(pool, "beta").await,
    );
    assert_ne!(alpha, beta);

    let grouped = |status: Option<&str>| {
        let st = st.clone();
        let status = status.map(str::to_string);
        async move {
            let mut params = json!({ "hours": 24, "source": "sentry", "kind": "event", "group": 1, "sort": "frequent" });
            if let Some(status) = status {
                params["status"] = json!(status);
            }
            dashboard::events(State(st), q(params))
                .await
                .expect("issues")
                .0
        }
    };
    let before = grouped(None).await;
    assert_eq!(
        before["items"].as_array().unwrap().len(),
        3,
        "three issues before the regroup: {before}"
    );

    let regrouped = admin::regroup(
        admin_of(&st).await,
        State(st.clone()),
        HeaderMap::new(),
        aq(),
        body(json!({ "canonical": alpha, "sources": [beta] })),
    )
    .await
    .expect("regroup")
    .0;
    assert_eq!(regrouped["merged"], json!(1));

    let after = grouped(None).await;
    let items = after["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "beta folded into alpha: {after}");
    assert_eq!(
        (
            items[0]["fingerprint"].as_str(),
            items[0]["count"].as_i64(),
            items[0]["users"].as_i64()
        ),
        (Some(alpha.as_str()), Some(3), Some(2)),
        "the canonical issue carries the source's events and users: {after}"
    );
    assert!(
        items.iter().all(|i| i["fingerprint"] != json!(beta)),
        "the source is no issue of its own: {after}"
    );

    // The canonical's state governs the folded group.
    sqlx::query("INSERT INTO telemetry.issue_state (fingerprint, status, updated_at) VALUES ($1, 'resolved', timezone('UTC', now()))").bind(&alpha).execute(pool).await.unwrap();
    let resolved = grouped(Some("resolved")).await;
    assert_eq!(resolved["items"].as_array().unwrap().len(), 1, "{resolved}");
    assert_eq!(resolved["items"][0]["count"], json!(3));

    // The events page and the stats under the canonical fingerprint read the sources' events too.
    let page = dashboard::events(
        State(st.clone()),
        q(json!({ "hours": 24, "fingerprint": alpha })),
    )
    .await
    .expect("events")
    .0;
    assert_eq!(page["items"].as_array().unwrap().len(), 3, "{page}");
    let page = dashboard::events(
        State(st.clone()),
        q(json!({ "hours": 24, "fingerprint": beta })),
    )
    .await
    .expect("events")
    .0;
    assert_eq!(
        page["items"].as_array().unwrap().len(),
        1,
        "a source still answers its own events: {page}"
    );
    let stats = dashboard::stats(
        State(st.clone()),
        q(json!({ "hours": 24, "fingerprint": alpha })),
    )
    .await
    .expect("stats")
    .0;
    assert_eq!(stats["total"], json!(3), "{stats}");
}
