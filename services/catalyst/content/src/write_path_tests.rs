//! The local write path through the content router, the way `serve` wires
//! it: the multipart deploy with the content validator attached (the LAND
//! leg over the `land_authz` index), the durable denylist on deploy and on
//! the reads, and the signed unpublish; the gates in front of both routes.
//!
//! The scene round trip runs on a database of its own (the LAND index's
//! cursor row is global, as in the validator's index test), created from
//! `CATALYST_TEST_DATABASE_URL` and dropped at the end; the profile
//! contract runs on the process's scratch database against a loopback
//! collections subgraph. Both skip without the variable.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use alloy::signers::{Signer as _, local::PrivateKeySigner};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode, header};
use bytes::Bytes;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tower::ServiceExt;

use super::tests::support::{TestNode, require_test_db};
use super::write_deployer::WriteDeployer;
use super::{ContentState, Settings, SharedState, SyncState, build_router, writes};
use crate::auth::{AuthLink, AuthLinkType, hash_bytes_v1};
use crate::http::Outbound;
use crate::land_authz::{LandAuthzState, events::AuthzEvent, indexer};
use crate::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};
use crate::storage::Storage;

/// Hardhat accounts #0 and #1: the LAND owner and a stranger.
const OWNER_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const STRANGER_KEY: &str = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

fn key(hex: &str) -> PrivateKeySigner {
    hex.parse().unwrap()
}

fn address(signer: &PrivateKeySigner) -> String {
    format!("{:#x}", signer.address())
}

/// An entity file signed by `signer`: its bytes, id and auth chain.
async fn signed_entity(signer: &PrivateKeySigner, entity: Value) -> (Bytes, String, Value) {
    let bytes = Bytes::from(serde_json::to_vec(&entity).unwrap());
    let id = hash_bytes_v1(&bytes);
    let signature = signer
        .sign_message(id.as_bytes())
        .await
        .unwrap()
        .to_string();
    let chain = json!([
        {"type": "SIGNER", "payload": address(signer), "signature": ""},
        {"type": "ECDSA_SIGNED_ENTITY", "payload": id, "signature": signature}
    ]);
    (bytes, id, chain)
}

fn scene(pointers: &[&str], files: &[(&str, &str)]) -> Value {
    json!({
        "version": "v3",
        "type": "scene",
        "pointers": pointers,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "content": files.iter().map(|(file, hash)| json!({"file": file, "hash": hash})).collect::<Vec<_>>(),
        "metadata": {
            "display": {"title": "write path e2e"},
            "main": "bin/index.js",
            "scene": {"base": pointers[0], "parcels": pointers},
        },
    })
}

/// The multipart form the SDK sends: `entityId`, `authChain` as JSON, and
/// every file as a part named by its hash.
fn deploy_request(
    path: &str,
    entity_id: &str,
    chain: &Value,
    files: &[(&str, &Bytes)],
) -> Request<Body> {
    let boundary = "catalyst-write-path-boundary";
    let mut body: Vec<u8> = Vec::new();
    let mut field = |name: &str, value: &[u8], filename: Option<&str>| {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match filename {
            Some(f) => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes(),
            ),
            None => body.extend_from_slice(format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes()),
        }
        body.extend_from_slice(value);
        body.extend_from_slice(b"\r\n");
    };
    field("entityId", entity_id.as_bytes(), None);
    field("authChain", chain.to_string().as_bytes(), None);
    for (hash, bytes) in files {
        field(hash, bytes, Some(hash));
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Request::post(path)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap()
}

/// `DELETE path` signed by `signer` over the 6.x payload.
async fn signed_delete(signer: &PrivateKeySigner, path: &str) -> Request<Body> {
    signed_delete_over(signer, path, path).await
}

/// `DELETE path` carrying a chain `signer` signed over `signed_path`.
async fn signed_delete_over(
    signer: &PrivateKeySigner,
    signed_path: &str,
    path: &str,
) -> Request<Body> {
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let metadata = r#"{"signer":"dcl:explorer"}"#;
    let payload = build_payload_v6("delete", signed_path, &timestamp, metadata);
    let signature = signer
        .sign_message(payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let chain = [
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: address(signer),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload,
            signature: Some(signature),
        },
    ];
    let mut req = Request::delete(path).body(Body::empty()).unwrap();
    let headers: &mut HeaderMap = req.headers_mut();
    headers.insert(
        AUTH_TIMESTAMP_HEADER,
        HeaderValue::from_str(&timestamp).unwrap(),
    );
    headers.insert(
        AUTH_METADATA_HEADER,
        HeaderValue::from_static(r#"{"signer":"dcl:explorer"}"#),
    );
    for (i, link) in chain.iter().enumerate() {
        headers.insert(
            HeaderName::from_bytes(format!("{AUTH_CHAIN_HEADER_PREFIX}{i}").as_bytes()).unwrap(),
            HeaderValue::from_str(&serde_json::to_string(link).unwrap()).unwrap(),
        );
    }
    req
}

async fn call(router: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, Bytes) {
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    (
        status,
        headers,
        to_bytes(resp.into_body(), 1 << 26).await.unwrap(),
    )
}

fn json_of(bytes: &Bytes) -> Value {
    serde_json::from_slice(bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(bytes).into_owned()))
}

async fn active_by_pointers(router: &Router, pointers: &[&str]) -> Value {
    let req = Request::post("/content/entities/active")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"pointers": pointers}).to_string()))
        .unwrap();
    let (status, _, body) = call(router, req).await;
    assert_eq!(status, StatusCode::OK);
    json_of(&body)
}

/// Attaches a deployer over the node's pool and store: the validator with
/// the given sources, the node's denylist.
fn attach(
    state: &SharedState,
    enabled: bool,
    land: Option<Arc<LandAuthzState>>,
    market: Option<Arc<crate::chain_index::ChainIndex>>,
) {
    let sources = super::validator::Sources::new(
        state.pool.clone(),
        state.storage.clone(),
        Outbound::for_tests(),
        None,
        land.clone(),
        market,
        super::validator::Settings::default(),
    );
    let deployer =
        WriteDeployer::with_optional_eip1654(state.pool.clone(), state.storage.clone(), None)
            .with_land_authz(land)
            .with_validator(Arc::new(sources))
            .with_denylist(state.denylist.clone());
    assert!(state.attach_writes(enabled, Arc::new(deployer)));
}

/// The gates, in order, on a node whose database is never reached: the
/// multipart extraction first (the stage-1 fixture's 400), then
/// `CONTENT_WRITES_ENABLED` (503, `Retry-After`), then the console's
/// read-only toggle (the old 403), then bootstrapping (the old 503). The
/// unpublish route has the same two gates.
#[tokio::test]
async fn the_write_routes_answer_their_gates_in_order() {
    let node = TestNode::without_db("write-gates").await;
    let router = node.router();
    let not_multipart = Request::post("/content/entities")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"hello":"world"}"#))
        .unwrap();
    let (status, _, body) = call(&router, not_multipart).await;
    assert_eq!(
        (status, &body[..]),
        (
            StatusCode::BAD_REQUEST,
            &b"Invalid `boundary` for `multipart/form-data` request"[..]
        )
    );

    let chain = json!([{"type": "SIGNER", "payload": "0xabc"}]);
    for path in ["/content/entities", "/entities"] {
        let (status, headers, body) =
            call(&router, deploy_request(path, "bafy", &chain, &[])).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(headers[header::RETRY_AFTER], "3600");
        assert_eq!(json_of(&body), json!({"error": writes::DISABLED_MESSAGE}));
    }
    let (status, _, _) = call(
        &router,
        Request::delete("/content/scenes/0,0")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "unpublish is gated by the same switch"
    );

    let disabled = TestNode::without_db("write-gates-off").await;
    attach(&disabled.state, false, None, None);
    let (status, _, _) = call(
        &disabled.router(),
        deploy_request("/entities", "bafy", &chain, &[]),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "an attached but disabled write path is still off"
    );

    attach(&node.state, true, None, None);
    node.state.set_read_only(true);
    let (status, _, body) = call(
        &router,
        deploy_request("/content/entities", "bafy", &chain, &[]),
    )
    .await;
    assert_eq!(
        (status, &body[..]),
        (StatusCode::FORBIDDEN, writes::READ_ONLY_MESSAGE.as_bytes())
    );
    let (status, _, body) = call(
        &router,
        Request::delete("/scenes/0,0").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(
        (status, &body[..]),
        (StatusCode::FORBIDDEN, writes::READ_ONLY_MESSAGE.as_bytes())
    );
    node.state.set_read_only(false);

    let (status, _, body) = call(
        &router,
        deploy_request("/content/entities", "bafy", &chain, &[]),
    )
    .await;
    assert_eq!(
        (status, json_of(&body)),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error": writes::BOOTSTRAPPING_MESSAGE})
        )
    );

    node.state.sync.set_state(SyncState::Syncing);
    let (status, _, body) = call(
        &router,
        deploy_request("/content/entities", "bafy", &json!([]), &[]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    let (status, _, body) = call(
        &router,
        Request::delete("/content/scenes/0,0")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        (status, json_of(&body)),
        (
            StatusCode::BAD_REQUEST,
            json!({"error": "Invalid Auth Chain"})
        ),
        "an unsigned unpublish"
    );
}

/// `SERVE_UNSYNCED` forges `Syncing` for the read contracts. While
/// `SYNC_SOURCE` names a peer (its default does) the node is an unsynced copy
/// of that peer and keeps the bootstrapping refusal; with no source at all (an
/// island or a journey node) it is its own authority and a deploy passes.
#[tokio::test]
async fn serve_unsynced_takes_deploys_only_with_no_sync_source() {
    for (i, (source, takes_deploys)) in [
        (None, false),
        (Some("https://peer.example/content"), false),
        (Some(""), true),
        (Some(" , "), true),
    ]
    .into_iter()
    .enumerate()
    {
        let mut argv = vec!["x", "--database-url", "postgres:///c", "--serve-unsynced"];
        argv.extend(source.map(|s| ["--sync-source", s]).into_iter().flatten());
        let config = crate::config::Config::parse_for_tests(&argv).unwrap();
        let node = TestNode::without_db(&format!("serve-unsynced-{i}")).await;
        attach(&node.state, true, None, None);
        crate::sync::worker::serve_unsynced(&config, &node.state.sync);
        assert_eq!(node.state.sync.state_name(), "Syncing", "{source:?}");

        let (status, _, body) = call(
            &node.router(),
            deploy_request("/content/entities", "bafy", &json!([]), &[]),
        )
        .await;
        if takes_deploys {
            assert_eq!(status, StatusCode::BAD_REQUEST, "{source:?}: {body:?}");
        } else {
            assert_eq!(
                (status, json_of(&body)),
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    json!({"error": writes::BOOTSTRAPPING_MESSAGE})
                ),
                "{source:?}"
            );
        }
    }
}

/// `SERVE_UNSYNCED_DEPLOY_TYPES` opens an unsynced copy to the types it
/// names (a development stack's own profiles): such a deploy passes the gate
/// to the deployer's validation, any other type keeps the bootstrapping 503.
#[tokio::test]
async fn an_unsynced_copy_takes_the_deploy_types_it_names() {
    let config = crate::config::Config::parse_for_tests(&[
        "x",
        "--database-url",
        "postgres:///c",
        "--serve-unsynced",
        "--sync-source",
        "https://peer.example/content",
        "--serve-unsynced-deploy-types",
        " Profile ,",
    ])
    .unwrap();
    let node = TestNode::without_db("serve-unsynced-types").await;
    attach(&node.state, true, None, None);
    crate::sync::worker::serve_unsynced(&config, &node.state.sync);
    assert!(!node.state.sync.takes_deploys());
    assert!(node.state.sync.takes_deploys_of("profile"));

    let entity = |kind: &str| {
        Bytes::from(
            json!({"version": "v3", "type": kind, "pointers": ["0x00000000000000000000000000000000000000aa"],
                   "timestamp": chrono::Utc::now().timestamp_millis(), "content": [], "metadata": {}})
            .to_string(),
        )
    };
    for (kind, taken) in [("profile", true), ("scene", false)] {
        let file = entity(kind);
        let id = crate::auth::hash_bytes_v1(&file);
        let (status, _, body) = call(
            &node.router(),
            deploy_request("/content/entities", &id, &json!([]), &[(&id, &file)]),
        )
        .await;
        let refused = (status, json_of(&body))
            == (
                StatusCode::SERVICE_UNAVAILABLE,
                json!({"error": writes::BOOTSTRAPPING_MESSAGE}),
            );
        assert_eq!(!refused, taken, "{kind}: {status} {body:?}");
        if taken {
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "the unsigned profile is validated: {body:?}"
            );
        }
    }
    let other = Bytes::from_static(b"{}");
    let (status, _, _) = call(
        &node.router(),
        deploy_request(
            "/content/entities",
            "bafynotthefile",
            &json!([]),
            &[("x", &other)],
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "no entity file, no type: refused"
    );
}

/// A switch that is on still waits for the restore stamps: a stale backup
/// stamp answers the content 503 naming the stamp and its age, on the
/// deploy and the unpublish alike, before the read-only and bootstrapping
/// gates; a fresh one lets the request on to them.
#[tokio::test]
async fn a_stale_restore_stamp_holds_content_writes_and_names_it() {
    use crate::shared::writes::restore::{
        lab::{HOUR, StampLab},
        scoped,
    };
    let node = TestNode::without_db("write-gates-held").await;
    let router = node.router();
    attach(&node.state, true, None, None);
    let chain = json!([{"type": "SIGNER", "payload": "0xabc"}]);
    let lab = StampLab::fresh();
    scoped(lab.stamps.clone(), async {
        let (_, _, body) = call(
            &router,
            deploy_request("/content/entities", "bafy", &chain, &[]),
        )
        .await;
        assert_eq!(
            json_of(&body),
            json!({"error": writes::BOOTSTRAPPING_MESSAGE}),
            "fresh stamps pass the request on to the next gate"
        );
        lab.advance(30 * HOUR);
        let held = "content writes are held on this node: the last complete backup (backup.json) is 31 h old, past WRITES_BACKUP_MAX_AGE_HOURS=26";
        let (status, headers, body) = call(
            &router,
            deploy_request("/content/entities", "bafy", &chain, &[]),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(headers[header::RETRY_AFTER], "3600");
        assert_eq!(json_of(&body), json!({"error": held}));
        let (status, _, body) = call(
            &router,
            Request::delete("/content/scenes/0,0")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(
            (status, json_of(&body)),
            (StatusCode::SERVICE_UNAVAILABLE, json!({"error": held}))
        );
    })
    .await;
}

/// Content stores blobs: a backup that left them out (`--no-blobs`) holds
/// its writes, naming the stamp.
#[tokio::test]
async fn a_backup_without_blobs_holds_content_writes() {
    use crate::shared::writes::restore::{Stamp, lab::StampLab, scoped};
    let node = TestNode::without_db("write-gates-no-blobs").await;
    let router = node.router();
    attach(&node.state, true, None, None);
    let chain = json!([{"type": "SIGNER", "payload": "0xabc"}]);
    let lab = StampLab::fresh();
    lab.write_json(
        Stamp::Backup,
        json!({
            "kind": "backup",
            "completed_at": (lab.now() - chrono::Duration::hours(1)).to_rfc3339(),
            "snapshot": "20261002T020000Z",
            "manifest_sha256": "ab",
            "blobs": { "included": false },
        }),
    );
    scoped(lab.stamps.clone(), async {
        let (status, _, body) = call(
            &router,
            deploy_request("/content/entities", "bafy", &chain, &[]),
        )
        .await;
        assert_eq!(
            (status, json_of(&body)),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                json!({"error": "content writes are held on this node: the last complete backup (backup.json) left the blobs out (backup.sh --no-blobs)"})
            )
        );
    })
    .await;
}

/// A database of its own, created from `CATALYST_TEST_DATABASE_URL` with
/// schema.sql applied; dropped by [`OwnDb::drop_now`].
struct OwnDb {
    admin: PgPool,
    name: String,
    pool: PgPool,
}

impl OwnDb {
    async fn create(tag: &str) -> Option<Self> {
        let base = std::env::var("CATALYST_TEST_DATABASE_URL").ok()?;
        let opts: PgConnectOptions = base.parse().unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.database("postgres"))
            .await
            .unwrap();
        let name = format!(
            "catalyst_test_{tag}_{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let mut url = url::Url::parse(&base).unwrap();
        url.set_path(&format!("/{name}"));
        let pool = crate::db::connect_pool(
            url.as_str(),
            &crate::db::PoolSettings {
                max_connections: 6,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        crate::db::apply_schema(&pool).await.unwrap();
        Some(Self { admin, name, pool })
    }

    async fn drop_now(self) {
        self.pool.close().await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await;
    }
}

/// The decimal token id of LAND (x, y), as the registry packs it.
fn land_token(x: i32, y: i32) -> String {
    use alloy_primitives::U256;
    let value: U256 = (U256::from(x as i128 as u128) << 128usize) | U256::from(y as i128 as u128);
    value.to_string()
}

/// Folds `owner`'s LAND at `parcels` into the index and publishes the fold.
async fn index_land(pool: &PgPool, owner: &str, parcels: &[(i32, i32)]) {
    let settings = crate::land_authz::Settings::for_tests();
    let start = settings.start_block as i64;
    indexer::ensure_cursor(pool, &settings).await.unwrap();
    let events: Vec<AuthzEvent> = parcels
        .iter()
        .enumerate()
        .map(|(i, (x, y))| AuthzEvent {
            block_number: start + 10,
            log_index: i as i32,
            block_time: (start + 10) * 12,
            token_address: crate::land_authz::LAND_REGISTRY_MAINNET.to_string(),
            kind: crate::land_authz::KIND_TRANSFER,
            token_id: Some(land_token(*x, *y)),
            estate_id: None,
            account: Some(owner.to_string()),
            operator: None,
            approved: None,
        })
        .collect();
    let mut conn = pool.acquire().await.unwrap();
    indexer::insert_events(&mut conn, &events).await.unwrap();
    indexer::set_cursor(&mut conn, crate::land_authz::NETWORK_MAINNET, start + 100)
        .await
        .unwrap();
    drop(conn);
    indexer::fold(pool, &settings, None).await.unwrap();
    assert!(
        indexer::mark_folded(
            pool,
            crate::land_authz::NETWORK_MAINNET,
            (start + 100) as u64
        )
        .await
        .unwrap()
    );
}

/// The old publish round trip, through the routes: the LAND owner deploys a
/// signed scene (`CONTENT_WRITES_ENABLED`, the validator's LAND leg over the
/// index), `/entities/active` returns it and `/contents/{hash}` serves its
/// files, `/audit` shows the local provenance; a stranger is refused on the
/// same parcel; the denylist hides and refuses; the stranger cannot
/// unpublish; the owner can, once, and the parcel is empty afterwards.
#[tokio::test]
async fn a_signed_scene_deploys_serves_and_unpublishes_end_to_end() {
    require_test_db!(Some(db) = OwnDb::create("contentw").await);
    let (owner, stranger) = (key(OWNER_KEY), key(STRANGER_KEY));
    let (x, y) = (37, -11);
    let parcel = format!("{x},{y}");
    index_land(&db.pool, &address(&owner), &[(x, y), (x + 1, y)]).await;

    let tmp = std::env::temp_dir().join(format!(
        "catalyst-write-path-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let storage = Arc::new(Storage::from_config(&tmp).await.unwrap());
    let state = Arc::new(ContentState::new(
        db.pool.clone(),
        Settings::for_tests(),
        storage.clone(),
    ));
    state.sync.set_state(SyncState::Syncing);
    let land = Arc::new(LandAuthzState::new(
        db.pool.clone(),
        Outbound::for_tests(),
        crate::land_authz::Settings::for_tests(),
        None,
    ));
    attach(&state, true, Some(land), None);
    let router = build_router(state.clone());

    let code = Bytes::from_static(b"export function onUpdate() {}");
    let code_hash = hash_bytes_v1(&code);
    let (entity_bytes, entity_id, chain) =
        signed_entity(&owner, scene(&[&parcel], &[("bin/index.js", &code_hash)])).await;
    state
        .deployments_cache
        .insert("stale".into(), Bytes::from_static(b"[]"));
    let (status, _, body) = call(
        &router,
        deploy_request(
            "/content/entities",
            &entity_id,
            &chain,
            &[(&entity_id, &entity_bytes), (&code_hash, &code)],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", json_of(&body));
    let created = json_of(&body)["creationTimestamp"].as_i64().unwrap();
    assert!((created - chrono::Utc::now().timestamp_millis()).abs() < 60_000);
    assert!(
        state.deployments_cache.is_empty(),
        "a committed deployment clears the history memo"
    );

    let active = active_by_pointers(&router, &[&parcel]).await;
    assert_eq!(active.as_array().unwrap().len(), 1, "{active}");
    assert_eq!(active[0]["id"], entity_id);
    assert_eq!(active[0]["pointers"], json!([parcel]));
    assert_eq!(
        active[0]["content"],
        json!([{"file": "bin/index.js", "hash": code_hash}])
    );
    assert_eq!(active[0]["metadata"]["scene"]["base"], parcel);
    for (hash, bytes) in [(&code_hash, &code), (&entity_id, &entity_bytes)] {
        let (status, _, served) = call(
            &router,
            Request::get(format!("/content/contents/{hash}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!((status, &served), (StatusCode::OK, bytes), "{hash}");
    }
    let (status, _, audit) = call(
        &router,
        Request::get(format!("/content/audit/scene/{entity_id}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let audit = json_of(&audit);
    assert_eq!(audit["localProvenance"]["status"], "active");
    assert_eq!(audit["localProvenance"]["signer"], address(&owner));

    let deployer = state.deployer().unwrap().clone();
    deployer.clear_rate_limits();
    let (bytes2, id2, chain2) = signed_entity(&stranger, scene(&[&parcel], &[])).await;
    let (status, headers, body) = call(
        &router,
        deploy_request("/entities", &id2, &chain2, &[(&id2, &bytes2)]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{}", json_of(&body));
    assert!(headers.get(header::RETRY_AFTER).is_none());
    let errors = json_of(&body)["errors"].clone();
    assert!(
        errors.to_string().contains("does not have access"),
        "{errors}"
    );

    // The denylist: a listed hash is not served and cannot be deployed
    // again; a listed entity drops out of the reads.
    state.denylist.operator_insert(&code_hash);
    let (status, _, _) = call(
        &router,
        Request::get(format!("/contents/{code_hash}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let neighbour = format!("{},{y}", x + 1);
    let (bytes3, id3, chain3) = signed_entity(
        &owner,
        scene(&[&neighbour], &[("bin/index.js", &code_hash)]),
    )
    .await;
    let (status, _, body) = call(
        &router,
        deploy_request("/content/entities", &id3, &chain3, &[(&id3, &bytes3)]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        json_of(&body),
        json!({"errors": [format!("These content files are denylisted on this node: {code_hash}")]})
    );
    state.denylist.operator_remove(&code_hash);
    state.denylist.operator_insert(&entity_id);
    assert_eq!(active_by_pointers(&router, &[&parcel]).await, json!([]));
    state.denylist.operator_remove(&entity_id);
    assert_eq!(
        active_by_pointers(&router, &[&parcel])
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Unpublish: the stranger holds no LAND there; the owner does, once.
    let path = format!("/content/scenes/{parcel}");
    let (status, _, body) = call(&router, signed_delete(&stranger, &path).await).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{}", json_of(&body));
    assert_eq!(
        json_of(&body)["error"],
        format!("The provided Eth Address does not have access to the following parcel: ({x},{y})")
    );
    let (status, _, body) = call(
        &router,
        signed_delete_over(&owner, "/content/scenes/0,0", &path).await,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a chain signed for another path: {}",
        json_of(&body)
    );

    state
        .deployments_cache
        .insert("stale".into(), Bytes::from_static(b"[]"));
    let (status, _, body) = call(&router, signed_delete(&owner, &path).await).await;
    assert_eq!(status, StatusCode::OK, "{}", json_of(&body));
    assert_eq!(
        json_of(&body),
        json!({"entityId": entity_id, "unpublished": true, "parcels": [parcel]})
    );
    assert!(state.deployments_cache.is_empty());
    assert_eq!(active_by_pointers(&router, &[&parcel]).await, json!([]));
    let (_, _, audit) = call(
        &router,
        Request::get(format!("/audit/scene/{entity_id}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(json_of(&audit)["localProvenance"]["status"], "unpublished");
    let (status, _, body) = call(&router, signed_delete(&owner, &path).await).await;
    assert_eq!(
        (status, json_of(&body)),
        (
            StatusCode::NOT_FOUND,
            json!({"error": format!("No locally published scene at {x},{y}.")})
        )
    );

    // The owner publishes again over the tombstone.
    deployer.clear_rate_limits();
    let (bytes4, id4, chain4) = signed_entity(&owner, scene(&[&parcel], &[])).await;
    let (status, _, body) = call(
        &router,
        deploy_request("/entities", &id4, &chain4, &[(&id4, &bytes4)]),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", json_of(&body));
    assert_eq!(active_by_pointers(&router, &[&parcel]).await[0]["id"], id4);

    drop((router, deployer, state));
    let _ = std::fs::remove_dir_all(&tmp);
    db.drop_now().await;
}

/// The collections subgraph on the loopback: `owned` is `(owner, item urn,
/// token id)`; every request is counted.
async fn serve_collections(owned: Vec<(String, String, String)>, hits: Arc<AtomicUsize>) -> String {
    use axum::extract::State;
    type Owned = (Vec<(String, String, String)>, Arc<AtomicUsize>);
    async fn handle(
        State(shared): State<Arc<Owned>>,
        axum::Json(body): axum::Json<Value>,
    ) -> axum::Json<Value> {
        let (owned, hits) = &*shared;
        hits.fetch_add(1, Ordering::SeqCst);
        let query = body["query"].as_str().unwrap_or_default();
        let owner = body["variables"]["owner"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let mine: Vec<(&String, &String)> = owned
            .iter()
            .filter(|(o, ..)| *o == owner)
            .map(|(_, urn, token)| (urn, token))
            .collect();
        if query.contains("ownedCopies") {
            let urns: Vec<&str> = body["variables"]["urns"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .collect();
            let rows: Vec<Value> = mine
                .iter()
                .filter(|(urn, _)| urns.contains(&urn.as_str()))
                .enumerate()
                .map(|(i, (urn, token))| json!({"id": format!("nft-{i}"), "urn": urn, "tokenId": token}))
                .collect();
            return axum::Json(json!({"data": {"nfts": rows}}));
        }
        let alias = regex::Regex::new(r#"p(\d+): nfts\(first: 1, where: \{ owner: \$owner, urn_starts_with: ("(?:[^"\\]|\\.)*") \}\)"#).unwrap();
        let mut data = serde_json::Map::new();
        for cap in alias.captures_iter(query) {
            let prefix: String = serde_json::from_str(&cap[2]).unwrap();
            let hit: Vec<Value> = mine
                .iter()
                .filter(|(urn, _)| urn.starts_with(&prefix))
                .map(|(urn, _)| json!({"urn": urn}))
                .take(1)
                .collect();
            data.insert(format!("p{}", &cap[1]), Value::Array(hit));
        }
        axum::Json(json!({"data": data}))
    }
    let app = Router::new()
        .route("/", axum::routing::post(handle))
        .with_state(Arc::new((owned, hits)));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/")
}

fn profile(owner: &str, wearables: &[&str]) -> Value {
    json!({
        "version": "v3",
        "type": "profile",
        "pointers": [owner],
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "content": [],
        "metadata": {"avatars": [{
            "name": "writepath", "hasClaimedName": false, "description": "",
            "avatar": {
                "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseMale",
                "wearables": wearables,
                "emotes": [{"slot": 0, "urn": "urn:decentraland:off-chain:base-emotes:wave"}]
            }
        }]},
    })
}

/// The old `profile_batch_no_overlay` contract on the write path: with no
/// escrow lease anywhere, a worn item counts for the wallet that holds that
/// token and for no one else (alice holds token 7 of item 1, bob token 9 of
/// item 2; a profile deployed now names each item with its token id, the
/// validator's token tier), another token of an item one holds is not
/// ownership, and a profile wearing only base items asks the subgraph
/// nothing.
#[tokio::test]
async fn a_profile_deploy_keeps_owner_scope_without_usage_grants() {
    require_test_db!(Some(pool) = crate::db::scratch::pool(4).await);
    let (alice, bob) = (PrivateKeySigner::random(), PrivateKeySigner::random());
    let contract =
        format!("0x{}", &uuid::Uuid::new_v4().simple().to_string()[..32]).to_string() + "00000000";
    let (item1, item2) = (
        format!("urn:decentraland:matic:collections-v2:{contract}:1"),
        format!("urn:decentraland:matic:collections-v2:{contract}:2"),
    );
    let (alices, bobs) = (format!("{item1}:7"), format!("{item2}:9"));
    let hits = Arc::new(AtomicUsize::new(0));
    let graph = serve_collections(
        vec![
            (address(&alice), item1.clone(), "7".to_string()),
            (address(&bob), item2.clone(), "9".to_string()),
        ],
        hits.clone(),
    )
    .await;
    let market = Arc::new(crate::chain_index::ChainIndex::new(
        Outbound::for_tests(),
        crate::chain_index::IndexSettings {
            eth_collections_subgraph_url: Some(graph.clone()),
            matic_collections_subgraph_url: Some(graph),
            ..crate::chain_index::IndexSettings::for_tests()
        },
    ));
    market.allow_loopback_for_tests();
    let granted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop.usage_grants WHERE grantee_address = ANY($1)",
    )
    .bind(vec![address(&alice), address(&bob)])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(granted, 0, "no overlay rows for either wallet");

    let tmp = std::env::temp_dir().join(format!(
        "catalyst-write-profile-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let storage = Arc::new(Storage::from_config(&tmp).await.unwrap());
    let state = Arc::new(ContentState::new(
        pool.clone(),
        Settings::for_tests(),
        storage,
    ));
    state.sync.set_state(SyncState::Syncing);
    attach(&state, true, None, Some(market));
    let router = build_router(state.clone());

    let deployer = state.deployer().unwrap().clone();
    let deploy = |signer: PrivateKeySigner, wearables: Vec<String>| {
        let router = router.clone();
        let deployer = deployer.clone();
        async move {
            deployer.clear_rate_limits();
            let refs: Vec<&str> = wearables.iter().map(String::as_str).collect();
            let (bytes, id, chain) =
                signed_entity(&signer, profile(&address(&signer), &refs)).await;
            let (status, _, body) = call(
                &router,
                deploy_request("/content/entities", &id, &chain, &[(&id, &bytes)]),
            )
            .await;
            (status, json_of(&body), id)
        }
    };

    let (status, body, _) = deploy(alice.clone(), vec![alices.clone(), bobs.clone()]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["errors"],
        json!([format!(
            "The following items ({bobs}) are not owned by the address {}.",
            address(&alice)
        )])
    );
    let (status, body, _) = deploy(bob.clone(), vec![alices.clone(), bobs.clone()]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["errors"],
        json!([format!(
            "The following items ({alices}) are not owned by the address {}.",
            address(&bob)
        )])
    );
    let other_token = format!("{item1}:8");
    let (status, body, _) = deploy(alice.clone(), vec![other_token.clone()]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["errors"],
        json!([format!(
            "The following items ({other_token}) are not owned by the address {}.",
            address(&alice)
        )])
    );

    let (status, body, alice_id) = deploy(alice.clone(), vec![alices.clone()]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, bob_id) = deploy(bob.clone(), vec![bobs.clone()]).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let before = hits.load(Ordering::SeqCst);
    let carol = PrivateKeySigner::random();
    let (status, body, carol_id) = deploy(
        carol.clone(),
        vec!["urn:decentraland:off-chain:base-avatars:eyes_00".to_string()],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        before,
        "base items are never asked about"
    );

    let mut scope: HashMap<String, String> = HashMap::new();
    for (id, who) in [(&alice_id, &alice), (&bob_id, &bob), (&carol_id, &carol)] {
        scope.insert(id.clone(), address(who));
    }
    for (id, who) in &scope {
        let active = active_by_pointers(&router, &[who]).await;
        assert_eq!(active[0]["id"], json!(id));
        assert!(
            super::land_publish::local_provenance(&pool, id)
                .await
                .unwrap()
                .is_none(),
            "only a local scene carries provenance"
        );
    }
    for (id, who) in scope {
        sqlx::query("DELETE FROM content.active_pointers WHERE pointer = $1")
            .bind(&who)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let _ = std::fs::remove_dir_all(&tmp);
    pool.close().await;
}
