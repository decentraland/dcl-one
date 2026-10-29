// from: catalyrst/crates/catalyrst-worlds/tests/federation_mirror_invariants.rs, catalyrst/crates/catalyrst-worlds/tests/federation_mirror_poll.rs, catalyrst/crates/catalyrst-worlds/tests/federation_mirror_routes.rs, catalyrst/crates/catalyrst-worlds/tests/audit_federation_holes.rs
//! The worlds federation read mirror against a real postgres and a stub peer: the
//! storage-level invariants (asserted through `information_schema` and real constraint
//! violations, so they outlive the Rust API), the poll path end to end (a peer that
//! lies, that answers 500, that sends ten megabytes, that goes away, that publishes a
//! name we hold locally), the four routes through the real router (the refresh route
//! authenticates before it contacts anybody, the mirror response carries no ownership
//! key), and the closed audit holes (de-admission revokes publication on both paths, a
//! peer file naming nobody does not sweep, the veto route authorizes before any
//! extractor answers, database error text stays off the public route).
//!
//! Needs CATALYST_TEST_DATABASE_URL; each test gets its own scratch database and drops
//! it. A test that skips says so on stderr; the pass tally is not evidence.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::extract::State as AxumState;
use axum::http::{Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use chrono::NaiveDateTime;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use catalyst::worlds::fed::config::WorldsFedConfig;
use catalyst::worlds::fed::names::{PeerId, RemoteWorldName};
use catalyst::worlds::fed::peers::{
    AdmissionOutcome, AdmittedPeer, PeerOmitted, WorldsFederationPeers,
};
use catalyst::worlds::fed::poll::{
    COLLISION_PROBE_UNAVAILABLE_PREFIX, PROBE_FAULT_PUBLIC_REASON, WorldsMirror,
};
use catalyst::worlds::fed::store::{RemoteWorld, RemoteWorldsComponent, Revocation, SweptBecause};
use catalyst::worlds::fed::{FedState, handlers, router};

#[path = "support/db.rs"]
mod db;

const ADMIN_TOKEN: &str = "fed-routes-admin";
const PEER: &str = "worlds.peer-operator.org";
const PEER_A: &str = "peer-a.dclone.org";
const PEER_B: &str = "peer-b.dclone.org";

fn skipped(property: &str) {
    eprintln!("skipping (no CATALYST_TEST_DATABASE_URL): unverified \u{2014} {property}");
}

macro_rules! db_or_skip {
    ($property:expr) => {
        match db::scratch_database().await {
            Some(s) => s,
            None => {
                skipped($property);
                return;
            }
        }
    };
}

// ── the stub peer ──────────────────────────────────────────────────────────

/// What the stub peer answers with. Each variant is a way a real peer has failed or
/// could lie.
#[derive(Clone)]
enum PeerBehaviour {
    Body(String),
    /// A well-formed listing, but only from the Nth request onward; 500 before that.
    FailsThenServes {
        after: usize,
        body: String,
    },
    Status(u16),
    /// 200 with a non-JSON content type. The bytes are valid JSON, so this isolates the
    /// content-type gate from the parser.
    WrongContentType(String),
    Oversized(usize),
    NotJson,
}

#[derive(Clone)]
struct StubState {
    behaviour: PeerBehaviour,
    hits: Arc<AtomicUsize>,
}

async fn stub_worlds(AxumState(state): AxumState<StubState>) -> Response {
    let n = state.hits.fetch_add(1, Ordering::SeqCst);
    match &state.behaviour {
        PeerBehaviour::Body(b) => {
            ([(header::CONTENT_TYPE, "application/json")], b.clone()).into_response()
        }
        PeerBehaviour::FailsThenServes { after, body } => {
            if n < *after {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            } else {
                ([(header::CONTENT_TYPE, "application/json")], body.clone()).into_response()
            }
        }
        PeerBehaviour::Status(s) => StatusCode::from_u16(*s).unwrap().into_response(),
        PeerBehaviour::WrongContentType(b) => {
            ([(header::CONTENT_TYPE, "text/html")], b.clone()).into_response()
        }
        PeerBehaviour::Oversized(n_bytes) => {
            let filler = "x".repeat(*n_bytes);
            let body =
                json!({ "worlds": [{ "name": "big.dcl.eth", "title": filler }] }).to_string();
            ([(header::CONTENT_TYPE, "application/json")], body).into_response()
        }
        PeerBehaviour::NotJson => (
            [(header::CONTENT_TYPE, "application/json")],
            "<html>nope</html>",
        )
            .into_response(),
    }
}

/// Counts **every** inbound request, not just `/worlds`, so a call that reaches the
/// peer at some other path is caught too.
async fn stub_any(AxumState(state): AxumState<StubState>) -> Response {
    state.hits.fetch_add(1, Ordering::SeqCst);
    StatusCode::NOT_FOUND.into_response()
}

struct StubPeer {
    addr: SocketAddr,
    hits: Arc<AtomicUsize>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl StubPeer {
    async fn start(behaviour: PeerBehaviour) -> Self {
        let hits = Arc::new(AtomicUsize::new(0));
        let state = StubState {
            behaviour,
            hits: hits.clone(),
        };
        let app = Router::new()
            .route("/worlds", get(stub_worlds))
            .fallback(get(stub_any))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
        });
        Self {
            addr,
            hits,
            shutdown: Some(tx),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

fn fed_config(max_response_bytes: u64) -> WorldsFedConfig {
    WorldsFedConfig {
        peers_file: Some(std::path::PathBuf::from("/nonexistent/peers.toml")),
        poll_interval_secs: 300,
        max_response_bytes,
        max_worlds_per_peer: 10_000,
        allow_insecure_loopback_peers: true,
        own_urls: Vec::new(),
    }
}

/// A peer certificate that clears every admission gate except the pinned root, which
/// the loopback opt-out excuses.
fn loopback_cert(peer_id: &str, worlds_url: &str) -> catalyst::worlds::fed::registry::PeerCert {
    catalyst::worlds::fed::registry::PeerCert {
        version: 1,
        peer_id: peer_id.to_string(),
        catalyst_url: "https://peer.dclone.org".to_string(),
        gossip_pubkey: [7u8; 32],
        mtls_root_pem: String::new(),
        dao_proposal: "https://snapshot.org/#/snapshot.dcl.eth/proposal/0xabc123".to_string(),
        added_at: "2026-07-01".to_string(),
        worlds_url: worlds_url.to_string(),
    }
}

fn admit_with(peer_id: &str, worlds_url: &str, cfg: &WorldsFedConfig) -> AdmittedPeer {
    match AdmittedPeer::admit(&loopback_cert(peer_id, worlds_url), cfg) {
        Ok(AdmissionOutcome::Admitted(p)) => p,
        other => panic!("fixture peer must be admitted, got {other:?}"),
    }
}

fn admit(peer_id: &str, worlds_url: &str) -> AdmittedPeer {
    admit_with(peer_id, worlds_url, &fed_config(4 * 1024 * 1024))
}

fn registry(peers: Vec<AdmittedPeer>) -> WorldsFederationPeers {
    WorldsFederationPeers::Admitted {
        path: std::path::PathBuf::from("/nonexistent/peers.toml"),
        peers,
        omitted: Vec::new(),
    }
}

fn listing(names: &[&str]) -> String {
    let worlds: Vec<_> = names
        .iter()
        .map(|n| {
            json!({ "name": n, "title": format!("{n} title"), "owner": "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef", "deployed_scenes": 2 })
        })
        .collect();
    json!({ "worlds": worlds, "total": worlds.len() }).to_string()
}

fn mirror(pool: &PgPool, cfg: WorldsFedConfig, peers: &WorldsFederationPeers) -> WorldsMirror {
    WorldsMirror::new(pool.clone(), cfg, peers)
}

fn now() -> NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

fn row(peer: &PeerId, name: &str) -> RemoteWorld {
    RemoteWorld {
        peer_id: peer.clone(),
        name: RemoteWorldName::from_operator_veto_path(name).expect("shaped name"),
        title: Some("mirrored".into()),
        description: None,
        content_rating: None,
        categories: None,
        thumbnail_hash: None,
        deployed_scenes: 1,
        last_deployed_at: None,
        observed_at: now(),
        hidden_since: None,
    }
}

async fn seed_local_world(pool: &PgPool, name: &str, owner: &str) {
    sqlx::query("INSERT INTO worlds.worlds (name, owner, title) VALUES ($1, $2, 'local') ON CONFLICT (name) DO NOTHING")
        .bind(name)
        .bind(owner)
        .execute(pool)
        .await
        .expect("seed local world");
}

async fn insert_remote(pool: &PgPool, peer: &str, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO worlds.federation_worlds (peer_id, world_name, title, deployed_scenes, observed_at)
         VALUES ($1, $2, 'from the peer', 0, $3)",
    )
    .bind(peer)
    .bind(name)
    .bind(now())
    .execute(pool)
    .await
    .map(|_| ())
}

async fn count(pool: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("count({table}): {e}"))
}

async fn columns_of(pool: &PgPool, table: &str) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT column_name FROM information_schema.columns WHERE table_schema = 'worlds' AND table_name = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .expect("introspect columns")
}

async fn mirrored_names(pool: &PgPool, peer: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT world_name FROM worlds.federation_worlds WHERE peer_id = $1 ORDER BY world_name",
    )
    .bind(peer)
    .fetch_all(pool)
    .await
    .expect("read mirror")
}

async fn status_row(
    pool: &PgPool,
    peer: &str,
) -> (Option<NaiveDateTime>, Option<NaiveDateTime>, Option<String>) {
    sqlx::query_as("SELECT last_attempt_at, last_success_at, last_error FROM worlds.federation_peer_status WHERE peer_id = $1")
        .bind(peer)
        .fetch_one(pool)
        .await
        .expect("status row")
}

fn build_app(pool: &PgPool, peers: WorldsFederationPeers) -> Router {
    let state = FedState::for_tests(
        pool.clone(),
        fed_config(4 * 1024 * 1024),
        peers,
        Some(ADMIN_TOKEN.to_string()),
    );
    router(state)
}

/// The federation routes merged with the local worlds read routes, so a test can
/// show a mirrored name is not answered under this origin.
async fn build_app_with_worlds(pool: &PgPool, peers: WorldsFederationPeers) -> Router {
    let tmp = std::env::temp_dir().join(format!(
        "catalyst-fed-routes-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let storage = Arc::new(catalyst::storage::Storage::from_config(&tmp).await.unwrap());
    let worlds = Arc::new(catalyst::worlds::WorldsState::new(
        pool.clone(),
        storage,
        catalyst::worlds::Settings::for_tests("http://fed.test"),
        None,
        catalyst::http::Outbound::for_tests(),
    ));
    build_app(pool, peers).merge(catalyst::worlds::router(worlds))
}

async fn call(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.expect("router responds");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn get_req(path: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .body(Body::empty())
        .unwrap()
}

fn refresh_req(auth: Option<&str>) -> Request<Body> {
    let mut req = Request::builder()
        .method("POST")
        .uri("/admin/federation/worlds/refresh");
    if let Some(h) = auth {
        req = req.header("authorization", h);
    }
    req.body(Body::empty()).unwrap()
}

fn bearer() -> String {
    format!("Bearer {ADMIN_TOKEN}")
}

// ══ storage-level invariants ════════════════════════════════════════════════

/// A peer's `/worlds` carries `owner` on every entry. The mirror drops it because
/// there is no column, struct field or bind parameter that could accept it -- not by
/// remembering to.
#[tokio::test]
async fn federation_worlds_has_no_column_that_could_hold_a_peer_ownership_claim() {
    let scratch =
        db_or_skip!("federation_worlds may have gained an owner/access/permissions column");
    let cols = columns_of(&scratch.pool, "federation_worlds").await;
    assert!(!cols.is_empty(), "worlds.federation_worlds does not exist");
    for forbidden in [
        "owner",
        "access",
        "permissions",
        "blocked_since",
        "deployer",
        "deployment_auth_chain",
        "single_player",
    ] {
        assert!(
            !cols.iter().any(|c| c == forbidden),
            "federation_worlds grew a `{forbidden}` column. A peer asserts this in-band on \
             every poll; the table exists in this shape so the assertion has nowhere to land. Columns: {cols:?}"
        );
    }
    scratch.teardown().await;
}

/// The hostile payload at the storage layer: every key a lying peer would add is
/// rejected by Postgres itself, not by a filter someone has to maintain.
#[tokio::test]
async fn every_forbidden_key_from_a_hostile_peer_payload_is_rejected_by_the_database() {
    let scratch = db_or_skip!("a forbidden peer-supplied column may now be insertable");
    for (col, val) in [
        ("owner", "'0xdeadbeef'"),
        ("access", "'{}'"),
        ("blocked_since", "now()"),
        ("deployer", "'0xdeadbeef'"),
    ] {
        let sql = format!(
            "INSERT INTO worlds.federation_worlds (peer_id, world_name, observed_at, {col}) \
             VALUES ('{PEER}', 'x.dcl.eth', timezone('UTC', now()), {val})"
        );
        let err = sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&scratch.pool)
            .await
            .expect_err(&format!("INSERT naming `{col}` succeeded \u{2014} a peer-supplied {col} now has somewhere to land"));
        let msg = err.to_string();
        assert!(
            msg.contains(col) || msg.contains("column"),
            "expected an undefined-column error for `{col}`, got: {msg}"
        );
    }
    scratch.teardown().await;
}

/// `world_permissions` attaches by FK to `worlds.worlds(name)`. A remote-only name has
/// no `worlds.worlds` row, so no ACL row can be created against it -- enforced by the
/// schema, not by a check in a handler.
#[tokio::test]
async fn an_acl_row_cannot_attach_to_a_remote_only_world() {
    let scratch = db_or_skip!("world_permissions may now be attachable to a mirrored world");
    insert_remote(&scratch.pool, PEER, "remote-only.dcl.eth")
        .await
        .expect("insert remote world");

    let err = sqlx::query(
        "INSERT INTO worlds.world_permissions (world_name, permission, address)
         VALUES ('remote-only.dcl.eth', 'deployment', '0x1111111111111111111111111111111111111111')",
    )
    .execute(&scratch.pool)
    .await
    .expect_err("an ACL row was created against a world that exists only as a peer's claim");
    assert!(
        err.to_string().contains("foreign key") || err.to_string().contains("violates"),
        "expected a foreign-key violation, got: {err}"
    );
    scratch.teardown().await;
}

#[tokio::test]
async fn federation_worlds_has_no_foreign_key_into_the_authoritative_tables() {
    let scratch = db_or_skip!("federation_worlds may have gained an FK to worlds(name)");
    let fks = sqlx::query_scalar::<_, String>(
        "SELECT tc.constraint_name FROM information_schema.table_constraints tc
         WHERE tc.table_schema = 'worlds' AND tc.table_name = 'federation_worlds' AND tc.constraint_type = 'FOREIGN KEY'",
    )
    .fetch_all(&scratch.pool)
    .await
    .expect("introspect fks");
    assert!(
        fks.is_empty(),
        "federation_worlds gained a FOREIGN KEY ({fks:?}). Coupling the mirror to worlds(name) \
         makes a peer's listing able to constrain, cascade into, or block writes on the authoritative table."
    );
    scratch.teardown().await;
}

#[tokio::test]
async fn mirroring_a_peer_writes_zero_rows_to_worlds_and_world_scenes() {
    let scratch = db_or_skip!("the mirror path may now write the authoritative worlds tables");
    seed_local_world(
        &scratch.pool,
        "local.dcl.eth",
        "0x1111111111111111111111111111111111111111",
    )
    .await;
    let worlds_before = count(&scratch.pool, "worlds.worlds").await;
    let scenes_before = count(&scratch.pool, "worlds.world_scenes").await;

    let stub = StubPeer::start(PeerBehaviour::Body(listing(&[
        "a.dcl.eth",
        "b.dcl.eth",
        "local.dcl.eth",
    ])))
    .await;
    let peer = admit(PEER, &stub.url());
    let peers = registry(vec![peer.clone()]);
    mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers)
        .poll_peer(&peer)
        .await
        .expect("poll");

    assert_eq!(
        count(&scratch.pool, "worlds.worlds").await,
        worlds_before,
        "mirroring changed the row count of `worlds.worlds`"
    );
    assert_eq!(
        count(&scratch.pool, "worlds.world_scenes").await,
        scenes_before,
        "mirroring changed the row count of `worlds.world_scenes`"
    );
    assert_eq!(count(&scratch.pool, "worlds.federation_worlds").await, 3);
    scratch.teardown().await;
}

/// The collision case at the storage layer: the same name held locally and claimed by
/// a peer are two independent rows in two independent tables. Neither upserts over the
/// other, and the local row's owner is untouched.
#[tokio::test]
async fn a_colliding_name_is_two_independent_rows_and_the_local_owner_survives() {
    let scratch = db_or_skip!("a peer's claim on a local name may now overwrite the local row");
    let local_owner = "0xcccccccccccccccccccccccccccccccccccccccc";
    seed_local_world(&scratch.pool, "collide.dcl.eth", local_owner).await;
    insert_remote(&scratch.pool, PEER, "collide.dcl.eth")
        .await
        .expect("peer claims the same name");

    let owner: Option<String> =
        sqlx::query_scalar("SELECT owner FROM worlds.worlds WHERE name = 'collide.dcl.eth'")
            .fetch_one(&scratch.pool)
            .await
            .expect("read local owner");
    assert_eq!(
        owner.as_deref(),
        Some(local_owner),
        "a peer's claim on a colliding name changed the local worlds.owner"
    );
    assert_eq!(count(&scratch.pool, "worlds.worlds").await, 1);
    assert_eq!(count(&scratch.pool, "worlds.federation_worlds").await, 1);
    scratch.teardown().await;
}

/// The mirror is invisible to the routes the ecosystem actually reads: a mirrored
/// name is not served by `/world/{name}/about` or `/index` under our origin (the old
/// `worlds-mirror` hazard of a mirrored world listed under our origin with no owner).
#[tokio::test]
async fn mirrored_rows_are_invisible_to_the_local_world_routes() {
    let scratch = db_or_skip!("mirrored rows may now be served by /index or /about");
    for n in ["one.dcl.eth", "two.dcl.eth"] {
        insert_remote(&scratch.pool, PEER, n)
            .await
            .expect("mirror insert");
    }
    let app = build_app_with_worlds(
        &scratch.pool,
        registry(vec![admit(PEER, "http://127.0.0.1:1/")]),
    )
    .await;
    let (status, index) = call(&app, get_req("/index")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        index["data"].as_array().map(Vec::len),
        Some(0),
        "a mirrored world was listed by /index: {index}"
    );
    let (status, _) = call(&app, get_req("/world/one.dcl.eth/about")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "this server must not vouch for a peer's world under its own origin"
    );
    scratch.teardown().await;
}

/// `federation_worlds` is keyed `(peer_id, world_name)`. Two peers claiming the same
/// name are two rows; one peer cannot displace another's claim.
#[tokio::test]
async fn two_peers_claiming_one_name_do_not_displace_each_other() {
    let scratch = db_or_skip!("one peer's listing may now overwrite another peer's rows");
    insert_remote(&scratch.pool, "worlds.peer-a.org", "shared.dcl.eth")
        .await
        .expect("peer a");
    insert_remote(&scratch.pool, "worlds.peer-b.org", "shared.dcl.eth")
        .await
        .expect("peer b");
    assert_eq!(count(&scratch.pool, "worlds.federation_worlds").await, 2);
    assert!(
        insert_remote(&scratch.pool, "worlds.peer-a.org", "shared.dcl.eth")
            .await
            .is_err(),
        "the PK no longer rejects a duplicate claim"
    );
    scratch.teardown().await;
}

/// A peer cannot mint a case-duplicate row that `lower()` reads resolve
/// non-deterministically: both keys are CHECKed lowercase.
#[tokio::test]
async fn a_case_variant_peer_name_violates_the_lowercase_check_constraint() {
    let scratch = db_or_skip!("federation_worlds may now accept mixed-case names from a peer");
    let err = insert_remote(&scratch.pool, PEER, "Collide.DCL.eth")
        .await
        .expect_err("a mixed-case world_name was accepted");
    assert!(
        err.to_string().contains("federation_worlds_name_lowercase")
            || err.to_string().contains("check"),
        "{err}"
    );
    let err = insert_remote(&scratch.pool, "Worlds.Peer-Operator.ORG", "x.dcl.eth")
        .await
        .expect_err("a mixed-case peer_id was accepted");
    assert!(
        err.to_string()
            .contains("federation_worlds_peer_id_lowercase")
            || err.to_string().contains("check"),
        "{err}"
    );
    scratch.teardown().await;
}

/// A poll that fails partway leaves the previous view intact: a half-replaced peer
/// view is worse than a stale one.
#[tokio::test]
async fn a_poll_that_fails_partway_leaves_the_previous_rows_intact() {
    let scratch = db_or_skip!("a failed poll may now leave a partially-replaced peer view");
    for n in ["keep-one.dcl.eth", "keep-two.dcl.eth"] {
        insert_remote(&scratch.pool, PEER, n).await.expect("seed");
    }
    let before = count(&scratch.pool, "worlds.federation_worlds").await;

    let mut tx = scratch.pool.begin().await.expect("begin");
    sqlx::query("DELETE FROM worlds.federation_worlds WHERE peer_id = $1 AND hidden_since IS NULL")
        .bind(PEER)
        .execute(&mut *tx)
        .await
        .expect("delete");
    let boom = sqlx::query("INSERT INTO worlds.federation_worlds (peer_id, world_name, deployed_scenes, observed_at) VALUES ($1, 'Bad-Case.dcl.eth', 0, $2)")
        .bind(PEER)
        .bind(now())
        .execute(&mut *tx)
        .await;
    assert!(boom.is_err(), "the malformed entry was accepted");
    tx.rollback().await.expect("rollback");

    assert_eq!(
        count(&scratch.pool, "worlds.federation_worlds").await,
        before,
        "a failed poll destroyed the previous view instead of leaving it stale"
    );
    scratch.teardown().await;
}

// ══ the poll path ═══════════════════════════════════════════════════════════

/// One name, held locally and claimed by a peer. Local wins *structurally*: the local
/// row keeps its owner, the mirrored row is separate, and the collision is reported.
#[tokio::test]
async fn a_name_held_locally_and_claimed_by_a_peer_resolves_local_wins_and_is_reported() {
    let scratch = db_or_skip!(
        "a peer claiming a locally-held world name leaves the local owner intact and is reported"
    );
    let owner = "0x1111111111111111111111111111111111111111";
    seed_local_world(&scratch.pool, "collide.dcl.eth", owner).await;

    let stub = StubPeer::start(PeerBehaviour::Body(listing(&[
        "collide.dcl.eth",
        "peer-only.dcl.eth",
    ])))
    .await;
    let peer = admit(PEER_A, &stub.url());
    let peers = registry(vec![peer.clone()]);
    let report = mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers)
        .poll_peer(&peer)
        .await
        .expect("poll succeeds");

    assert_eq!(report.worlds_observed, 2);
    assert_eq!(
        report.collisions.checked(),
        Some(&["collide.dcl.eth".to_string()][..]),
        "the collision must be reported"
    );
    assert_eq!(report.collisions.unavailable_reason(), None);

    let local_owner: Option<String> =
        sqlx::query_scalar("SELECT owner FROM worlds.worlds WHERE name = 'collide.dcl.eth'")
            .fetch_one(&scratch.pool)
            .await
            .unwrap();
    assert_eq!(local_owner.as_deref(), Some(owner));
    assert_eq!(
        count(&scratch.pool, "worlds.worlds").await,
        1,
        "the mirror did not add a row to `worlds.worlds`"
    );
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_A).await,
        vec![
            "collide.dcl.eth".to_string(),
            "peer-only.dcl.eth".to_string()
        ]
    );
    assert!(
        !columns_of(&scratch.pool, "federation_worlds")
            .await
            .iter()
            .any(|c| c == "owner")
    );
    scratch.teardown().await;
}

/// What a poll says when it **could not look**: the probe decides nothing, so its
/// failure does not fail the poll -- but it must render as unknown, never as "no
/// collisions", in the report and in the stored status row.
#[tokio::test]
async fn a_probe_that_could_not_run_reports_unknown_rather_than_no_collisions() {
    let scratch =
        db_or_skip!("a failed collision probe is reported as unknown, never as an empty list");
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&["probe.dcl.eth"]))).await;
    let peer = admit(PEER_A, &stub.url());
    let peers = registry(vec![peer.clone()]);
    let m = mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers);

    let clean = m.poll_peer(&peer).await.expect("the clean poll");
    assert_eq!(
        clean.collisions.checked(),
        Some(&[][..]),
        "knowledge of an absence"
    );
    let (_, clean_success, clean_error) = status_row(&scratch.pool, PEER_A).await;
    assert!(clean_success.is_some());
    assert_eq!(clean_error, None, "a clean poll records no error");

    sqlx::query("DROP TABLE worlds.worlds CASCADE")
        .execute(&scratch.pool)
        .await
        .expect("drop the local worlds table");
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let report = m
        .poll_peer(&peer)
        .await
        .expect("a probe failure must not fail a poll that already wrote its rows");
    assert_eq!(
        report.collisions.checked(),
        None,
        "a failed probe must not present as a checked, empty list"
    );
    let reason = report
        .collisions
        .unavailable_reason()
        .expect("an unavailable probe carries why");
    assert!(
        reason.to_lowercase().contains("worlds"),
        "the reason must name the failure, got {reason:?}"
    );
    assert!(
        report.collisions.to_string().starts_with("unknown ("),
        "{}",
        report.collisions
    );
    assert_eq!(report.worlds_observed, 1);
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_A).await,
        vec!["probe.dcl.eth".to_string()]
    );

    let (_, success, last_error) = status_row(&scratch.pool, PEER_A).await;
    assert!(
        success >= clean_success,
        "the fetch and the write succeeded, so the mirror is current"
    );
    let stored = last_error.expect("the unavailable probe is stored, not only logged");
    assert!(
        stored.starts_with(COLLISION_PROBE_UNAVAILABLE_PREFIX),
        "the stored note must say which half failed, got {stored:?}"
    );
    assert!(
        stored.ends_with(PROBE_FAULT_PUBLIC_REASON),
        "and carry the public reason, not the sqlx text: {stored:?}"
    );
    scratch.teardown().await;
}

#[tokio::test]
async fn two_peers_claiming_one_name_are_two_rows_and_a_poll_touches_only_its_own() {
    let scratch = db_or_skip!("a poll of one peer replaces only that peer's rows");
    let stub_a = StubPeer::start(PeerBehaviour::Body(listing(&[
        "shared.dcl.eth",
        "a-only.dcl.eth",
    ])))
    .await;
    let stub_b = StubPeer::start(PeerBehaviour::Body(listing(&[
        "shared.dcl.eth",
        "b-only.dcl.eth",
    ])))
    .await;
    let peer_a = admit(PEER_A, &stub_a.url());
    let peer_b = admit(PEER_B, &stub_b.url());
    let peers = registry(vec![peer_a.clone(), peer_b.clone()]);
    let m = mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers);
    m.poll_peer(&peer_a).await.expect("poll a");
    m.poll_peer(&peer_b).await.expect("poll b");
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_A).await,
        vec!["a-only.dcl.eth".to_string(), "shared.dcl.eth".to_string()]
    );
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_B).await,
        vec!["b-only.dcl.eth".to_string(), "shared.dcl.eth".to_string()]
    );

    let mut stub_a2 = StubPeer::start(PeerBehaviour::Body(listing(&["a-only.dcl.eth"]))).await;
    let peer_a2 = admit(PEER_A, &stub_a2.url());
    let peers2 = registry(vec![peer_a2.clone()]);
    mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers2)
        .poll_peer(&peer_a2)
        .await
        .expect("re-poll a");
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_A).await,
        vec!["a-only.dcl.eth".to_string()]
    );
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_B).await,
        vec!["b-only.dcl.eth".to_string(), "shared.dcl.eth".to_string()],
        "polling A must not disturb B"
    );
    stub_a2.stop();
    scratch.teardown().await;
}

/// One stub, every lie at once: ownership claims have nowhere to land, a name shaped
/// like a path traversal is refused and counted, and the good rows survive both.
#[tokio::test]
async fn a_lying_peer_loses_its_claims_and_its_bad_rows_without_losing_its_good_ones() {
    let scratch = db_or_skip!("a payload carrying owner/access/permissions stores none of them");
    let body = json!({
        "total": 4,
        "worlds": [
            { "name": "honest.dcl.eth",
              "owner": "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
              "access": { "type": "unrestricted" },
              "permissions": { "deployment": { "type": "allow-list", "wallets": ["0xdead"] } },
              "blocked_since": null,
              "deployer": "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef" },
            { "name": "../../etc/passwd" },
            { "name": "has spaces.dcl.eth" },
            { "name": "also-honest.dcl.eth" }
        ]
    })
    .to_string();
    let stub = StubPeer::start(PeerBehaviour::Body(body)).await;
    let peer = admit(PEER_A, &stub.url());
    let peers = registry(vec![peer.clone()]);
    let report = mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers)
        .poll_peer(&peer)
        .await
        .expect("poll succeeds");
    assert_eq!(report.worlds_observed, 2);
    assert_eq!(
        report.entries_skipped, 2,
        "each refused name is counted, not silently dropped"
    );
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_A).await,
        vec![
            "also-honest.dcl.eth".to_string(),
            "honest.dcl.eth".to_string()
        ]
    );
    let dumped: String = sqlx::query_scalar(
        "SELECT coalesce(string_agg(r::text, ' '), '') FROM worlds.federation_worlds r",
    )
    .fetch_one(&scratch.pool)
    .await
    .unwrap();
    assert!(
        !dumped.to_lowercase().contains("0xdeadbeef"),
        "a peer-asserted wallet reached storage: {dumped}"
    );
    scratch.teardown().await;
}

#[tokio::test]
async fn a_duplicate_name_in_one_listing_does_not_abort_the_poll() {
    let scratch = db_or_skip!("a peer listing one name twice does not abort the transaction");
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&[
        "dup.dcl.eth",
        "other.dcl.eth",
        "DUP.dcl.eth",
    ])))
    .await;
    let peer = admit(PEER_A, &stub.url());
    let peers = registry(vec![peer.clone()]);
    let report = mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers)
        .poll_peer(&peer)
        .await
        .expect("poll succeeds");
    assert_eq!(report.worlds_observed, 2);
    assert_eq!(report.entries_skipped, 1);
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_A).await,
        vec!["dup.dcl.eth".to_string(), "other.dcl.eth".to_string()]
    );
    scratch.teardown().await;
}

/// For each way a peer can fail, the previous rows survive and the peer is recorded as
/// failed: an empty listing is indistinguishable from "this peer holds no worlds".
#[tokio::test]
async fn every_peer_failure_retains_the_previous_rows_and_is_recorded_as_stale() {
    let scratch =
        db_or_skip!("each peer failure retains prior rows instead of emptying the mirror");
    let good = StubPeer::start(PeerBehaviour::Body(listing(&["kept.dcl.eth"]))).await;
    let peer = admit(PEER_A, &good.url());
    let peers = registry(vec![peer.clone()]);
    mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers)
        .poll_peer(&peer)
        .await
        .expect("the good poll");
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_A).await,
        vec!["kept.dcl.eth".to_string()]
    );
    let (_, first_success, _) = status_row(&scratch.pool, PEER_A).await;
    assert!(first_success.is_some());

    let cases: Vec<(&str, PeerBehaviour, &str)> = vec![
        (
            "HTTP 500",
            PeerBehaviour::Status(500),
            "peer answered HTTP 500",
        ),
        (
            "HTTP 404",
            PeerBehaviour::Status(404),
            "peer answered HTTP 404",
        ),
        (
            "non-JSON content type",
            PeerBehaviour::WrongContentType(listing(&["evil.dcl.eth"])),
            "expected JSON",
        ),
        (
            "JSON content type, non-JSON body",
            PeerBehaviour::NotJson,
            "not a worlds listing",
        ),
        (
            "body over the byte cap",
            PeerBehaviour::Oversized(300_000),
            "refused before parse",
        ),
    ];
    for (label, behaviour, expected_fragment) in cases {
        let mut stub = StubPeer::start(behaviour).await;
        let cfg = fed_config(64 * 1024);
        let peer = admit_with(PEER_A, &stub.url(), &cfg);
        let peers = registry(vec![peer.clone()]);
        let err = mirror(&scratch.pool, cfg, &peers)
            .poll_peer(&peer)
            .await
            .expect_err(&format!("{label} must fail the poll"));
        let msg = err.to_string();
        assert!(
            msg.contains(expected_fragment),
            "{label}: expected {expected_fragment:?} in {msg:?}"
        );
        assert_eq!(
            mirrored_names(&scratch.pool, PEER_A).await,
            vec!["kept.dcl.eth".to_string()],
            "{label} emptied the mirror"
        );
        let (attempt, success, last_error) = status_row(&scratch.pool, PEER_A).await;
        assert_eq!(
            success, first_success,
            "{label} must not advance last_success_at \u{2014} staleness is the signal"
        );
        assert!(attempt >= success, "{label}: the attempt is recorded");
        assert!(
            last_error.is_some(),
            "{label} must be recorded so the staleness has a reason"
        );
        stub.stop();
    }

    let mut dead = StubPeer::start(PeerBehaviour::Body(listing(&["gone.dcl.eth"]))).await;
    let dead_url = dead.url();
    dead.stop();
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let peer = admit(PEER_A, &dead_url);
    let peers = registry(vec![peer.clone()]);
    let err = mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers)
        .poll_peer(&peer)
        .await
        .expect_err("an unreachable peer must fail the poll");
    assert!(err.to_string().starts_with("transport:"), "{err}");
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_A).await,
        vec!["kept.dcl.eth".to_string()]
    );
    scratch.teardown().await;
}

/// A peer that has never answered has no rows *and* no `last_success_at`. Together the
/// two stop a consumer reading "no worlds" out of "no contact".
#[tokio::test]
async fn a_peer_that_never_answered_is_distinguishable_from_a_peer_with_no_worlds() {
    let scratch = db_or_skip!("never-contacted and genuinely-empty peers are distinguishable");
    let mut dead = StubPeer::start(PeerBehaviour::Body(String::new())).await;
    let dead_url = dead.url();
    dead.stop();
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let peer_a = admit(PEER_A, &dead_url);
    let empty = StubPeer::start(PeerBehaviour::Body(
        json!({ "worlds": [], "total": 0 }).to_string(),
    ))
    .await;
    let peer_b = admit(PEER_B, &empty.url());
    let peers = registry(vec![peer_a.clone(), peer_b.clone()]);
    let m = mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers);
    let _ = m.poll_peer(&peer_a).await;
    m.poll_peer(&peer_b)
        .await
        .expect("the empty peer polls fine");

    assert!(mirrored_names(&scratch.pool, PEER_A).await.is_empty());
    assert!(mirrored_names(&scratch.pool, PEER_B).await.is_empty());
    let (_, a_success, a_error) = status_row(&scratch.pool, PEER_A).await;
    let (_, b_success, b_error) = status_row(&scratch.pool, PEER_B).await;
    assert!(
        a_success.is_none(),
        "a peer we never reached has no success time"
    );
    assert!(a_error.is_some(), "and has a recorded reason");
    assert!(
        b_success.is_some(),
        "a peer that answered empty HAS a success time"
    );
    assert!(b_error.is_none(), "and no error");
    scratch.teardown().await;
}

#[tokio::test]
async fn a_recovered_peer_replaces_its_stale_rows() {
    let scratch = db_or_skip!("a peer that recovers replaces its stale rows and clears last_error");
    let stub = StubPeer::start(PeerBehaviour::FailsThenServes {
        after: 1,
        body: listing(&["fresh.dcl.eth"]),
    })
    .await;
    let peer = admit(PEER_A, &stub.url());
    let peers = registry(vec![peer.clone()]);
    let m = mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers);
    m.poll_peer(&peer).await.expect_err("first poll fails");
    assert!(mirrored_names(&scratch.pool, PEER_A).await.is_empty());
    let report = m.poll_peer(&peer).await.expect("second poll succeeds");
    assert_eq!(report.worlds_observed, 1);
    assert_eq!(
        mirrored_names(&scratch.pool, PEER_A).await,
        vec!["fresh.dcl.eth".to_string()]
    );
    let (_, _, last_error) = status_row(&scratch.pool, PEER_A).await;
    assert_eq!(last_error, None, "a success clears the recorded failure");
    assert!(stub.hits() >= 2);
    scratch.teardown().await;
}

/// `hidden_since` is ours. The poller's DELETE spares vetoed rows and its UPDATE arm
/// does not name the column, so a peer cannot un-hide itself by re-listing.
#[tokio::test]
async fn a_vetoed_row_survives_a_poll_and_stays_out_of_the_published_listing() {
    let scratch = db_or_skip!("hidden_since survives a poll and excludes the row from the mirror");
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&[
        "hidden.dcl.eth",
        "shown.dcl.eth",
    ])))
    .await;
    let peer = admit(PEER_A, &stub.url());
    let peers = registry(vec![peer.clone()]);
    let m = mirror(&scratch.pool, fed_config(4 * 1024 * 1024), &peers);
    m.poll_peer(&peer).await.expect("first poll");

    let name = RemoteWorldName::from_operator_veto_path("hidden.dcl.eth").expect("a plain name");
    assert!(
        m.store()
            .set_hidden(peer.peer_id(), &name, true, now())
            .await
            .expect("veto applies")
    );

    m.poll_peer(&peer).await.expect("second poll");
    let still_hidden: Option<NaiveDateTime> = sqlx::query_scalar(
        "SELECT hidden_since FROM worlds.federation_worlds WHERE peer_id = $1 AND world_name = $2",
    )
    .bind(PEER_A)
    .bind("hidden.dcl.eth")
    .fetch_one(&scratch.pool)
    .await
    .unwrap();
    assert!(
        still_hidden.is_some(),
        "a peer re-listing a world must not clear our veto"
    );

    let (published, total) = m
        .store()
        .list_mirror(&peers, Some(peer.peer_id()), 100, 0)
        .await
        .expect("list");
    assert_eq!(total, 1);
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].name.as_peer_reported_str(), "shown.dcl.eth");

    assert!(
        m.store()
            .set_hidden(peer.peer_id(), &name, false, now())
            .await
            .unwrap()
    );
    let (published, _) = m
        .store()
        .list_mirror(&peers, Some(peer.peer_id()), 100, 0)
        .await
        .unwrap();
    assert_eq!(published.len(), 2);

    let (page2, total) = m.store().list_mirror(&peers, None, 1, 1).await.unwrap();
    assert_eq!(
        (page2.len(), total),
        (1, 2),
        "the second page still reports the filtered total"
    );
    let (past_end, total) = m.store().list_mirror(&peers, None, 10, 5).await.unwrap();
    assert_eq!(
        (past_end.len(), total),
        (0, 2),
        "an offset past the end reports the total, not zero"
    );
    scratch.teardown().await;
}

/// The row cap truncates and *says so*.
#[tokio::test]
async fn the_row_cap_marks_the_peer_truncated_rather_than_silently_shortening() {
    let scratch =
        db_or_skip!("the per-peer row cap records `truncated` rather than shortening quietly");
    let names: Vec<String> = (0..50).map(|i| format!("w{i:03}.dcl.eth")).collect();
    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&refs))).await;
    let mut cfg = fed_config(4 * 1024 * 1024);
    cfg.max_worlds_per_peer = 10;
    let peer = admit_with(PEER_A, &stub.url(), &cfg);
    let peers = registry(vec![peer.clone()]);
    let report = mirror(&scratch.pool, cfg, &peers)
        .poll_peer(&peer)
        .await
        .expect("poll succeeds");
    assert_eq!(report.worlds_observed, 10);
    assert!(report.truncated);
    let truncated: bool = sqlx::query_scalar(
        "SELECT truncated FROM worlds.federation_peer_status WHERE peer_id = $1",
    )
    .bind(PEER_A)
    .fetch_one(&scratch.pool)
    .await
    .unwrap();
    assert!(
        truncated,
        "the cap is visible on the wire, not only in a log"
    );
    assert_eq!(mirrored_names(&scratch.pool, PEER_A).await.len(), 10);
    scratch.teardown().await;
}

/// The URL the poller builds comes from the registry and nothing else.
#[tokio::test]
async fn the_listing_url_is_built_from_the_registry_and_hits_only_the_registered_host() {
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&["x.dcl.eth"]))).await;
    let peer = admit(PEER_A, &stub.url());
    let url = peer.worlds_listing_url(500, 0);
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert_eq!(url.port(), Some(stub.addr.port()));
    assert_eq!(url.path(), "/worlds");
    let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(q.get("limit").map(String::as_str), Some("500"));
    assert_eq!(q.get("offset").map(String::as_str), Some("0"));
    assert_eq!(
        url.as_str().matches("//").count(),
        1,
        "the path is fixed in one place; nothing appends a peer-chosen segment"
    );
}

// ══ the routes ══════════════════════════════════════════════════════════════

/// An unauthenticated refresh is refused and the peer never sees a request -- proven
/// by the peer's own counter, not by reading the handler.
#[tokio::test]
async fn an_unauthenticated_refresh_is_refused_before_any_peer_is_contacted() {
    let scratch = db_or_skip!("an unauthenticated POST to the refresh route reaches zero peers");
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&["a.dcl.eth"]))).await;
    let app = build_app(&scratch.pool, registry(vec![admit(PEER_A, &stub.url())]));

    for headers in [
        None,
        Some("Bearer wrong"),
        Some("Basic anything"),
        Some(ADMIN_TOKEN),
    ] {
        let (status, _) = call(&app, refresh_req(headers)).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "unauthenticated refresh must be refused (auth header: {headers:?})"
        );
        assert_eq!(
            stub.hits(),
            0,
            "the peer was contacted by an unauthenticated caller (auth header: {headers:?})"
        );
    }

    let (status, body) = call(&app, refresh_req(Some(&bearer()))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["polled"][0]["ok"], json!(true));
    assert_eq!(body["polled"][0]["worldsObserved"], json!(1));
    assert_eq!(
        stub.hits(),
        1,
        "the authenticated call is the only one that got through"
    );
    scratch.teardown().await;
}

/// The veto route authorizes before any extractor can answer: an anonymous caller is
/// 403 whatever the body or Content-Type (the body is `Bytes`, parsed after the check).
#[tokio::test]
async fn the_veto_route_authorizes_before_any_extractor_can_answer() {
    let scratch = db_or_skip!("the veto route authorizes before any extractor can answer");
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&["a.dcl.eth"]))).await;
    let app = build_app(
        &scratch.pool,
        registry(vec![admit("peer.example.org", &stub.url())]),
    );
    let uri = "/admin/federation/worlds/peer.example.org/x.dcl.eth/hidden";

    for (label, content_type, body) in [
        ("no content-type, not json", None, "not json"),
        (
            "well-formed json, wrong schema",
            Some("application/json"),
            r#"{"hidden":"yes"}"#,
        ),
        (
            "declared json, unparseable",
            Some("application/json"),
            "{{{",
        ),
        ("empty body", Some("application/json"), ""),
        (
            "well-formed and valid",
            Some("application/json"),
            r#"{"hidden":true}"#,
        ),
    ] {
        let mut req = Request::builder().method("PUT").uri(uri);
        if let Some(ct) = content_type {
            req = req.header(header::CONTENT_TYPE, ct);
        }
        let (status, _) = call(&app, req.body(Body::from(body)).unwrap()).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "an anonymous caller must be refused by authz, not answered by an extractor \u{2014} case: {label}"
        );
    }
    assert_eq!(stub.hits(), 0);

    let (status, body) = call(
        &app,
        Request::builder()
            .method("PUT")
            .uri(uri)
            .header(header::AUTHORIZATION, bearer())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"hidden":"yes"}"#))
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an authenticated admin sending a body the schema rejects still gets an error: {body}"
    );

    let (status, _) = call(
        &app,
        Request::builder()
            .method("PUT")
            .uri(uri)
            .header(header::AUTHORIZATION, bearer())
            .body(Body::from(r#"{"hidden":true}"#))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no such mirrored row yet");
    scratch.teardown().await;
}

/// The published mirror carries the peer's world and none of the peer's claims, and a
/// mirrored name is not answered by this node's own world routes.
#[tokio::test]
async fn the_published_mirror_is_peer_qualified_and_carries_no_ownership_key() {
    let scratch = db_or_skip!(
        "the mirror response is qualified by peerId and contains no owner/access/permissions key"
    );
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&[
        "remote-one.dcl.eth",
        "remote-two.dcl.eth",
    ])))
    .await;
    let app =
        build_app_with_worlds(&scratch.pool, registry(vec![admit(PEER_A, &stub.url())])).await;

    let (status, _) = call(&app, refresh_req(Some(&bearer()))).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = call(&app, get_req("/federation/worlds/mirror")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], json!(2));
    for w in body["worlds"].as_array().expect("worlds array") {
        assert_eq!(
            w["peerId"],
            json!(PEER_A),
            "every row names the peer that said it"
        );
        assert!(w.get("owner").is_none());
        assert!(
            w["observedAt"]
                .as_str()
                .is_some_and(|t| t.ends_with("+00:00")),
            "UTC on the wire: {w}"
        );
    }
    let rendered = body.to_string();
    for forbidden in [
        "owner",
        "access",
        "permission",
        "blocked",
        "deployer",
        "singlePlayer",
        "0xdeadbeef",
    ] {
        assert!(
            !rendered.to_lowercase().contains(&forbidden.to_lowercase()),
            "the mirror response leaked {forbidden:?}:\n{rendered}"
        );
    }
    assert_eq!(body["peers"][0]["peerId"], json!(PEER_A));
    assert_eq!(body["peers"][0]["status"]["hasEverSucceeded"], json!(true));

    let (status, index) = call(&app, get_req("/index")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        index["data"].as_array().map(Vec::len),
        Some(0),
        "a mirrored world must not appear in the local index"
    );
    let (status, _) = call(&app, get_req("/world/remote-one.dcl.eth/about")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "this server must not vouch for a peer's world under its own origin"
    );
    scratch.teardown().await;
}

/// A collision probe that could not run must not reach the refresh JSON as an empty
/// collision list. Both states are exercised against one route, so the assertion is
/// that they *differ*.
#[tokio::test]
async fn a_refresh_reports_an_unavailable_collision_probe_as_null_not_as_an_empty_list() {
    let scratch =
        db_or_skip!("the refresh JSON distinguishes `no collisions` from `we could not check`");
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&["probe-a.dcl.eth"]))).await;
    let app = build_app(&scratch.pool, registry(vec![admit(PEER_A, &stub.url())]));

    let (status, body) = call(&app, refresh_req(Some(&bearer()))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["polled"][0]["ok"], json!(true));
    assert_eq!(
        body["polled"][0]["localNameCollisions"],
        json!([]),
        "a probe that ran and found nothing reports an empty list"
    );
    assert_eq!(body["polled"][0]["localNameCollisionsError"], json!(null));

    sqlx::query("DROP TABLE worlds.worlds CASCADE")
        .execute(&scratch.pool)
        .await
        .expect("drop the local worlds table");
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let (status, body) = call(&app, refresh_req(Some(&bearer()))).await;
    assert_eq!(status, StatusCode::OK);
    let result = &body["polled"][0];
    assert_eq!(
        result["ok"],
        json!(true),
        "the fetch and the write succeeded; the probe decides nothing and does not fail the poll"
    );
    assert_eq!(result["worldsObserved"], json!(1));
    assert_eq!(
        result["localNameCollisions"],
        json!(null),
        "an unavailable probe must be absent, never an empty list: {result}"
    );
    assert!(
        result["localNameCollisionsError"]
            .as_str()
            .is_some_and(|e| !e.is_empty()),
        "and it must say why: {result}"
    );

    let (status, peers_body) = call(&app, get_req("/federation/worlds/peers")).await;
    assert_eq!(status, StatusCode::OK);
    let status_block = &peers_body["peers"][0]["status"];
    assert_eq!(status_block["hasEverSucceeded"], json!(true));
    let last_error = status_block["lastError"]
        .as_str()
        .expect("the unavailable probe is recorded, not only logged");
    assert!(
        last_error.starts_with(COLLISION_PROBE_UNAVAILABLE_PREFIX),
        "{last_error:?}"
    );
    for leak in [
        "relation ",
        "at line ",
        "sqlx",
        "federation_worlds",
        "\"worlds\"",
    ] {
        assert!(
            !last_error.contains(leak),
            "the public reason must carry no internal detail; found {leak:?} in: {last_error}"
        );
    }
    scratch.teardown().await;
}

/// A peer whose poll failed reports no collision list, rather than an empty one.
#[tokio::test]
async fn a_refresh_of_an_unreachable_peer_reports_no_collision_list_rather_than_an_empty_one() {
    let scratch = db_or_skip!("a failed poll reports localNameCollisions: null with a reason");
    let app = build_app(
        &scratch.pool,
        registry(vec![admit("unreachable.dclone.org", "http://127.0.0.1:1")]),
    );
    let (status, body) = call(&app, refresh_req(Some(&bearer()))).await;
    assert_eq!(status, StatusCode::OK);
    let result = &body["polled"][0];
    assert_eq!(result["ok"], json!(false));
    assert!(
        result["error"].as_str().is_some_and(|e| !e.is_empty()),
        "a failed poll says why: {result}"
    );
    assert_eq!(result["localNameCollisions"], json!(null));
    assert_eq!(
        result["localNameCollisionsError"],
        json!(handlers::NOT_PROBED_POLL_FAILED)
    );
    scratch.teardown().await;
}

/// The peers route reports the allowlist and never the secrets in it.
#[tokio::test]
async fn the_peers_route_reports_the_allowlist_without_its_secrets() {
    let scratch = db_or_skip!("the peers route omits mtls_root_pem and gossip_pubkey");
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&["a.dcl.eth"]))).await;
    let app = build_app(&scratch.pool, registry(vec![admit(PEER_A, &stub.url())]));
    let (status, body) = call(&app, get_req("/federation/worlds/peers")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["configured"], json!(true));
    assert_eq!(body["peers"][0]["peerId"], json!(PEER_A));
    assert_eq!(
        body["peers"][0]["status"]["hasEverSucceeded"],
        json!(false),
        "before any poll, a peer is 'never succeeded' rather than 'empty'"
    );
    assert_eq!(body["peers"][0]["insecureLoopback"], json!(true));
    let rendered = body.to_string().to_lowercase();
    for secret in ["mtls", "pem", "gossip", "pubkey", "-----begin"] {
        assert!(
            !rendered.contains(secret),
            "the peers route leaked {secret:?}:\n{rendered}"
        );
    }
    scratch.teardown().await;
}

/// Unconfigured is a 503 naming the variable, on every route -- never an empty list.
#[tokio::test]
async fn every_federation_route_answers_503_when_federation_is_not_configured() {
    let scratch = db_or_skip!(
        "all four federation routes answer 503 naming WORLDS_FED_PEERS_FILE when unconfigured"
    );
    let app = build_app(&scratch.pool, WorldsFederationPeers::NotConfigured);
    for (method, uri, body) in [
        ("GET", "/federation/worlds/peers", None),
        ("GET", "/federation/worlds/mirror", None),
        (
            "POST",
            "/admin/federation/worlds/refresh",
            Some(String::new()),
        ),
        (
            "PUT",
            "/admin/federation/worlds/peer-a.dclone.org/x.dcl.eth/hidden",
            Some(r#"{"hidden":true}"#.to_string()),
        ),
    ] {
        let mut req = Request::builder().method(method).uri(uri);
        if uri.starts_with("/admin/") {
            req = req
                .header("authorization", bearer())
                .header("content-type", "application/json");
        }
        let (status, payload) = call(
            &app,
            req.body(body.map(Body::from).unwrap_or_else(Body::empty))
                .unwrap(),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {uri} must 503 when unconfigured, not return an empty list"
        );
        assert!(
            payload.to_string().contains("WORLDS_FED_PEERS_FILE"),
            "{method} {uri} must name the variable that is unset: {payload}"
        );
        assert_eq!(
            payload["ok"],
            json!(false),
            "the old worlds error envelope: {payload}"
        );
    }
    scratch.teardown().await;
}

/// An unknown peer id is a 404, not an empty listing that reads as a healthy peer
/// holding nothing.
#[tokio::test]
async fn an_unknown_peer_filter_is_a_404_not_an_empty_listing() {
    let scratch = db_or_skip!("?peer= for a non-admitted id is a 404, not an empty list");
    let stub = StubPeer::start(PeerBehaviour::Body(listing(&["a.dcl.eth"]))).await;
    let app = build_app(&scratch.pool, registry(vec![admit(PEER_A, &stub.url())]));
    let (status, _) = call(
        &app,
        get_req("/federation/worlds/mirror?peer=someone-else.example.org"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = call(
        &app,
        get_req(&format!(
            "/federation/worlds/mirror?peer={}",
            PEER_A.to_ascii_uppercase()
        )),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the peer filter folds case like the registry"
    );
    assert_eq!(body["total"], json!(0), "admitted but not yet polled");
    assert_eq!(body["peers"][0]["status"]["hasEverSucceeded"], json!(false));
    scratch.teardown().await;
}

// ══ the closed audit holes ══════════════════════════════════════════════════

/// A peer removed from the file is unlisted, unaddressable, **and unpublished**, and
/// the two federation routes agree about it.
#[tokio::test]
async fn a_revoked_peers_worlds_stop_being_published_and_the_two_routes_agree() {
    let scratch = db_or_skip!("removing a peer from the allowlist stops publishing its worlds");
    let pool = scratch.pool.clone();
    let store = RemoteWorldsComponent::new(pool.clone());

    let admitted = admit("revoked-peer.org", "http://127.0.0.1:1/");
    store
        .replace_peer_worlds(
            admitted.peer_id(),
            &[
                row(admitted.peer_id(), "kept.dcl.eth"),
                row(admitted.peer_id(), "alsokept.dcl.eth"),
            ],
        )
        .await
        .expect("mirror the peer");
    store
        .record_success(admitted.peer_id(), 2, 0, false, now())
        .await
        .expect("record success");

    let survivor = admit("kept-peer.org", "http://127.0.0.1:2/");
    store
        .replace_peer_worlds(
            survivor.peer_id(),
            &[row(survivor.peer_id(), "survivor.dcl.eth")],
        )
        .await
        .expect("mirror the surviving peer");

    let after_revocation = registry(vec![survivor.clone()]);
    let revocation = store
        .revoke_peers_no_longer_admitted(&after_revocation, now())
        .await
        .expect("the boot sweep runs");
    assert_eq!(
        revocation.worlds_deleted(),
        2,
        "both of the revoked peer's rows are destroyed, and only those two"
    );
    assert_eq!(revocation.revoked_peer_ids(), vec!["revoked-peer.org"]);

    let app = build_app(&pool, after_revocation);
    let (s, peers) = call(&app, get_req("/federation/worlds/peers")).await;
    assert_eq!(s, StatusCode::OK);
    let listed: Vec<&str> = peers["peers"]
        .as_array()
        .expect("peers array")
        .iter()
        .map(|p| p["peerId"].as_str().expect("peerId"))
        .collect();
    assert_eq!(
        listed,
        vec!["kept-peer.org"],
        "the revoked peer must not be in the allowlist"
    );

    let (s, _) = call(
        &app,
        get_req("/federation/worlds/mirror?peer=revoked-peer.org"),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::NOT_FOUND,
        "a revoked peer is not addressable by name"
    );

    let (s, mirror) = call(&app, get_req("/federation/worlds/mirror")).await;
    assert_eq!(s, StatusCode::OK);
    let published: Vec<&str> = mirror["worlds"]
        .as_array()
        .expect("worlds array")
        .iter()
        .map(|w| w["peerId"].as_str().expect("peerId"))
        .collect();
    assert_eq!(
        published,
        vec!["kept-peer.org"],
        "a peer removed from the allowlist must not be published: {mirror}"
    );
    assert_eq!(
        mirror["total"], 1,
        "`total` is filtered by the same predicate as `worlds`"
    );
    let status_lines: Vec<&str> = mirror["peers"]
        .as_array()
        .expect("peers block")
        .iter()
        .map(|p| p["peerId"].as_str().expect("peerId"))
        .collect();
    assert_eq!(status_lines, listed, "both routes render from one peer set");

    let tomb: (Option<NaiveDateTime>, i64, Option<NaiveDateTime>) = sqlx::query_as(
        "SELECT deadmitted_at, deadmitted_worlds_deleted, last_success_at FROM worlds.federation_peer_status WHERE peer_id = 'revoked-peer.org'",
    )
    .fetch_one(&pool)
    .await
    .expect("the de-admitted peer still has a status row");
    assert!(
        tomb.0.is_some(),
        "deadmitted_at records that the DELETE happened"
    );
    assert_eq!(tomb.1, 2, "and how much was destroyed");
    assert!(
        tomb.2.is_some(),
        "along with when we last successfully heard from the peer"
    );

    let readmitted = store
        .revoke_peers_no_longer_admitted(&registry(vec![survivor, admitted]), now())
        .await
        .expect("sweep again");
    let Revocation::Swept { readmitted, .. } = readmitted else {
        panic!("a named allowlist sweeps")
    };
    assert_eq!(
        readmitted,
        vec!["revoked-peer.org".to_string()],
        "re-admission clears the tombstone"
    );
    scratch.teardown().await;
}

/// Mechanism independence: publication must not rest on the boot sweep or the read
/// filter alone. This test deliberately **never calls the sweep** and asserts the
/// route publishes nothing regardless.
#[tokio::test]
async fn a_revoked_peers_rows_are_unpublishable_even_if_the_boot_sweep_never_ran() {
    let scratch =
        db_or_skip!("the read path refuses de-admitted rows without help from the boot sweep");
    let pool = scratch.pool.clone();
    let store = RemoteWorldsComponent::new(pool.clone());
    let ghost = admit("ghost-peer.org", "http://127.0.0.1:1/");
    store
        .replace_peer_worlds(ghost.peer_id(), &[row(ghost.peer_id(), "ghost.dcl.eth")])
        .await
        .expect("mirror the peer");

    let after_revocation = registry(Vec::new());
    assert_eq!(
        count(&pool, "worlds.federation_worlds").await,
        1,
        "the sweep was deliberately not run"
    );
    let (rows, total) = store
        .list_mirror(&after_revocation, None, 100, 0)
        .await
        .expect("list");
    assert!(
        rows.is_empty() && total == 0,
        "a stored row for a de-admitted peer must not be publishable by the read path"
    );

    let app = build_app(&pool, after_revocation);
    let (s, mirror) = call(&app, get_req("/federation/worlds/mirror")).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(mirror["worlds"].as_array().map(Vec::len), Some(0));
    assert_eq!(mirror["total"], 0);
    scratch.teardown().await;
}

/// An unset `WORLDS_FED_PEERS_FILE` is not a revocation, and a local operator veto
/// survives de-admission.
#[tokio::test]
async fn the_sweep_spares_an_unconfigured_server_and_spares_a_local_veto() {
    let scratch = db_or_skip!(
        "the sweep destroys nothing when there is no allowlist, and spares vetoed rows"
    );
    let pool = scratch.pool.clone();
    let store = RemoteWorldsComponent::new(pool.clone());
    let peer = admit("veto-peer.org", "http://127.0.0.1:1/");
    store
        .replace_peer_worlds(
            peer.peer_id(),
            &[
                row(peer.peer_id(), "vetoed.dcl.eth"),
                row(peer.peer_id(), "plain.dcl.eth"),
            ],
        )
        .await
        .expect("mirror the peer");
    let vetoed = RemoteWorldName::from_operator_veto_path("vetoed.dcl.eth").expect("shaped");
    assert!(
        store
            .set_hidden(peer.peer_id(), &vetoed, true, now())
            .await
            .expect("veto applies")
    );

    let unconfigured = store
        .revoke_peers_no_longer_admitted(&WorldsFederationPeers::NotConfigured, now())
        .await
        .expect("the sweep runs");
    assert_eq!(
        unconfigured.worlds_deleted(),
        0,
        "unsetting WORLDS_FED_PEERS_FILE is a configuration state, not a DAO revocation"
    );
    assert_eq!(count(&pool, "worlds.federation_worlds").await, 2);
    let (rows, total) = store
        .list_mirror(&WorldsFederationPeers::NotConfigured, None, 100, 0)
        .await
        .expect("list");
    assert!(
        rows.is_empty() && total == 0,
        "with no adjudicated allowlist, nothing is publishable"
    );

    let revoked = store
        .revoke_peers_no_longer_admitted(
            &registry(vec![admit("still-admitted.org", "http://127.0.0.1:1/")]),
            now(),
        )
        .await
        .expect("the sweep runs");
    assert_eq!(
        revoked.worlds_deleted(),
        1,
        "only the un-vetoed row is destroyed"
    );
    let remaining: Vec<String> =
        sqlx::query_scalar("SELECT world_name FROM worlds.federation_worlds ORDER BY world_name")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        remaining,
        vec!["vetoed.dcl.eth".to_string()],
        "the operator's veto survives"
    );
    scratch.teardown().await;
}

fn write_peer_file(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("catalyst-fed-audit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write peer file");
    path
}

/// A peer file whose table header is misspelled loads as an allowlist naming nobody --
/// and the boot sweep refuses to act on it, so the mirror survives the typo.
#[tokio::test]
async fn a_peer_file_with_the_section_header_mistyped_does_not_sweep_the_mirror() {
    let scratch = db_or_skip!(
        "a mistyped section header loads as empty, and the sweep declines to act on it"
    );
    let store = RemoteWorldsComponent::new(scratch.pool.clone());
    let peer = admit(PEER_A, "http://127.0.0.1:1/");
    store
        .replace_peer_worlds(peer.peer_id(), &[row(peer.peer_id(), "kept.dcl.eth")])
        .await
        .expect("mirror the peer");

    for (name, body) in [
        (
            "mistyped.toml",
            "[[peers]]\nversion = 1\npeer_id = \"peer-a.dclone.org\"\ncatalyst_url = \"https://peer-a.dclone.org/content\"\n",
        ),
        ("zero-bytes.toml", ""),
    ] {
        let path = write_peer_file(name, body);
        let peers = WorldsFederationPeers::load_file(&path, &fed_config(4 * 1024 * 1024))
            .expect("a zero-entry file still loads");
        assert!(peers.is_configured() && peers.peers().is_empty() && !peers.names_any_peer());
        let outcome = store
            .revoke_peers_no_longer_admitted(&peers, now())
            .await
            .expect("the sweep must not error");
        assert!(
            matches!(outcome, Revocation::NoAllowlistToEnforce),
            "{name}: an allowlist admitting nobody must not sweep, got {outcome:?}"
        );
        assert_eq!(
            count(&scratch.pool, "worlds.federation_worlds").await,
            1,
            "{name}: declining the sweep must leave the rows in place"
        );
    }
    scratch.teardown().await;
}

/// A peer that is **in** the file but carries no `worlds_url` is `Omitted`; the sweep
/// deletes its rows and reports it as an omission, not a de-admission.
#[tokio::test]
async fn a_peer_omitted_for_having_no_worlds_url_is_swept_as_an_omission_not_a_de_admission() {
    let scratch =
        db_or_skip!("an omitted peer's sweep is reported as an omission, not as a de-admission");
    let store = RemoteWorldsComponent::new(scratch.pool.clone());
    let peer = admit("still-in-the-file.dclone.org", "http://127.0.0.1:1/");
    let peer_id = peer.peer_id().clone();
    store
        .replace_peer_worlds(&peer_id, &[row(&peer_id, "kept.dcl.eth")])
        .await
        .expect("mirror the peer");

    let swept = store
        .revoke_peers_no_longer_admitted(
            &WorldsFederationPeers::Admitted {
                path: std::path::PathBuf::from("/etc/catalyst/federation-peers.toml"),
                peers: Vec::new(),
                omitted: vec![PeerOmitted::NoWorldsUrl {
                    peer_id: peer_id.as_str().to_string(),
                }],
            },
            now(),
        )
        .await
        .expect("the boot sweep runs");
    assert_eq!(
        swept.worlds_deleted(),
        1,
        "the rows go \u{2014} an omitted peer is not a worlds peer"
    );
    assert_eq!(swept.revoked_peer_ids(), vec![peer_id.as_str()]);
    let Revocation::Swept { revoked, .. } = &swept else {
        panic!("a file that names a peer sweeps: {swept:?}")
    };
    assert_eq!(
        revoked[0].because,
        SweptBecause::StillListedButRunsNoWorldsServer
    );
    scratch.teardown().await;
}

/// The unavailable probe is still reported on the public route; the raw database text
/// is not.
#[tokio::test]
async fn a_database_error_text_is_not_published_on_the_public_peers_route() {
    let scratch = db_or_skip!("internal database error text does not reach the public peers route");
    let store = RemoteWorldsComponent::new(scratch.pool.clone());
    let peer = admit("leaky.dclone.org", "http://127.0.0.1:1/");
    store
        .record_success(peer.peer_id(), 2, 0, false, now())
        .await
        .expect("record the successful fetch");
    store
        .record_failure(
            peer.peer_id(),
            &format!("{COLLISION_PROBE_UNAVAILABLE_PREFIX}{PROBE_FAULT_PUBLIC_REASON}"),
            now(),
        )
        .await
        .expect("restate the unavailable probe");

    let app = build_app(&scratch.pool, registry(vec![peer]));
    let (status, body) = call(&app, get_req("/federation/worlds/peers")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the peers route is public by design"
    );
    let last_error = body["peers"][0]["status"]["lastError"]
        .as_str()
        .expect("the unavailable probe is reported");
    assert!(
        last_error.contains("collision probe unavailable"),
        "{last_error}"
    );
    for leak in [
        "relation ",
        "at line ",
        "sqlx",
        "federation_worlds",
        "\"worlds\"",
    ] {
        assert!(
            !last_error.contains(leak),
            "the public reason must carry no internal detail; found {leak:?} in: {last_error}"
        );
    }
    assert!(
        body["peers"][0]["status"]["lastSuccessAt"].is_string(),
        "current rows AND an unchecked thing about them"
    );
    scratch.teardown().await;
}

/// The two escape hatches -- the peer-reported bytes and the local-name constructor --
/// must never be nameable in the same file under `worlds/fed/`, so no launder of any
/// length has a file to live in. (The local constructor lives in `world_identity.rs` of
/// `catalyst-comms`, outside the module; the lib gate in `fed/wire.rs` bans it under
/// `fed/` line by line.)
#[test]
fn the_two_escape_hatches_never_meet_under_the_federation_module() {
    let hatch = concat!("as_peer_reported", "_str");
    let ctors = [
        concat!("WorldName::", "parse("),
        concat!("WorldName::", "personal("),
    ];
    let fed = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/src/fed");
    let mut seen = 0;
    for entry in std::fs::read_dir(&fed).expect("worlds/src/fed is readable") {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        seen += 1;
        let body = std::fs::read_to_string(&path).unwrap();
        let code: Vec<&str> = body
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !(t.starts_with("//") || t.starts_with('*') || t.starts_with("#!"))
            })
            .collect();
        let has_ctor = code.iter().any(|l| ctors.iter().any(|c| l.contains(c)));
        assert!(
            !has_ctor,
            "{} mints a local world name inside the federation module",
            path.display()
        );
        let has_hatch = code.iter().any(|l| l.contains(hatch));
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        if has_hatch {
            assert!(
                ["names.rs", "wire.rs", "store.rs", "handlers.rs"].contains(&file.as_str()),
                "{file} reaches the peer-reported bytes"
            );
        }
    }
    assert!(
        seen >= 8,
        "the source walk found {seen} files; test is inert"
    );
    let wire = std::fs::read_to_string(fed.join("wire.rs")).unwrap();
    assert!(
        wire.contains("mints a local world name inside the federation module"),
        "the lib gate no longer bans the local constructor inside fed/"
    );
}
