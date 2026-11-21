//! The wearable validator's work as creators, curators and the validator
//! meet it, through the router as `serve` mounts it: a standard collection's
//! first publication and a creator's changes submission record a validation
//! request; the validator claims it, extends its lease and posts a verdict,
//! each call signed with the shared secret; a leased job abandons, expires
//! and deletes requests. Each test has a database of its own and drives the
//! node's clock.

use std::cell::Cell;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{
    FakeCommittee, FakeOracle, OWNER_KEY, committee_index, contents_of, save_collection,
    saved_item, signed_map, temp_storage, unique, upload, wallet,
};
use crate::shared::validator_signature::{SIGNATURE_HEADER, TIMESTAMP_HEADER, sign};

pub(super) const SECRET: &str = "example-validator-secret-0123456789abcdef";
const OTHER_SECRET: &str = "example-other-secret-0123456789abcdefghij";
const SLACK_TOKEN: &str = "xoxb-example123";
const CLAIMS: &str = "/v1/validator/claims";
const UNAUTHORIZED: &str = r#"{"data":{},"error":"Unauthorized","ok":false}"#;

thread_local! {
    static NOW: Cell<i64> = const { Cell::new(0) };
    static SENT: Cell<i64> = const { Cell::new(0) };
}

pub(super) fn clock() -> DateTime<Utc> {
    DateTime::from_timestamp_millis(NOW.with(Cell::get)).unwrap()
}

pub(super) fn set_clock(at: DateTime<Utc>) {
    NOW.with(|now| now.set(at.timestamp_millis()));
}

pub(super) fn advance(by: Duration) {
    set_clock(clock() + by);
}

fn at(iso: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(iso)
        .unwrap()
        .with_timezone(&Utc)
}

fn iso(t: DateTime<Utc>) -> String {
    crate::http::iso(&t)
}

pub(super) fn address(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

#[derive(Clone, Default)]
pub(super) struct FakeSlack {
    texts: Arc<Mutex<Vec<String>>>,
}

impl FakeSlack {
    pub(super) async fn start(&self) -> String {
        let fake = self.clone();
        let app = Router::new().route(
            "/api/chat.postMessage",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let fake = fake.clone();
                async move {
                    fake.texts
                        .lock()
                        .unwrap()
                        .push(body["text"].as_str().unwrap_or_default().to_string());
                    axum::Json(json!({ "ok": true }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/api")
    }

    pub(super) fn texts(&self) -> Vec<String> {
        let mut texts = self.texts.lock().unwrap().clone();
        texts.sort();
        texts
    }
}

#[derive(Clone)]
pub(super) struct Options {
    pub(super) secret: Option<&'static str>,
    pub(super) writes: bool,
    pub(super) storage: Option<Arc<crate::storage::Storage>>,
    pub(super) unbounded: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            secret: Some(SECRET),
            writes: true,
            storage: None,
            unbounded: false,
        }
    }
}

pub(super) struct Node {
    pub(super) pool: PgPool,
    pub(super) storage: Arc<crate::storage::Storage>,
    pub(super) app: Router,
    pub(super) state: crate::SharedState,
    pub(super) oracle: FakeOracle,
    pub(super) slack: FakeSlack,
}

/// The `builder` flags document as the flags mirror stores it, with
/// `builder-auto-curation` holding `value`; `None` stores no document.
pub(super) async fn set_flag(pool: &PgPool, value: Option<Value>) {
    sqlx::query("DELETE FROM explore_api.feature_flags WHERE app = 'builder'")
        .execute(pool)
        .await
        .unwrap();
    if let Some(value) = value {
        sqlx::query(
            "INSERT INTO explore_api.feature_flags (app, document, fetched_at) VALUES ('builder', $1, timezone('UTC', now()))",
        )
        .bind(json!({ "flags": { "builder-auto-curation": value }, "variants": {} }))
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn set_local_flag(pool: &PgPool, on: Option<bool>) {
    match on {
        Some(on) => crate::shared::feature_flags::set_local_flag(
            pool,
            "builder",
            "builder-auto-curation",
            on,
        )
        .await
        .unwrap(),
        None => {
            crate::shared::feature_flags::clear_local_flag(pool, "builder", "builder-auto-curation")
                .await
                .unwrap()
        }
    }
}

pub(super) async fn own_pool() -> Option<PgPool> {
    let pool = crate::db::scratch::isolated_pool(4).await?;
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    set_flag(&pool, Some(json!(true))).await;
    FakeCommittee::with(&[]).publish(&pool).await;
    Some(pool)
}

pub(super) async fn node_over(pool: PgPool, options: Options) -> Node {
    let oracle = FakeOracle::default();
    let slack = FakeSlack::default();
    let committee = FakeCommittee::with(&[]);
    let storage = match options.storage {
        Some(storage) => storage,
        None => temp_storage().await,
    };
    let mut state = crate::State::new(
        pool.clone(),
        storage.clone(),
        Arc::default(),
        dcl_http::Outbound::for_tests(),
        Some(committee_index(&committee).await),
        None,
        crate::Settings {
            writes_enabled: options.writes,
            polygon_rpc_url: Some(oracle.url().await),
            slack: Some(crate::curation::slack::Slack {
                token: SLACK_TOKEN.into(),
                channel: "curation-example".into(),
                api: slack.start().await,
            }),
            links: crate::curation::slack::Links {
                creators_url: Some("https://creators.example.org".into()),
                builder_url: None,
            },
            validator_secret: options.secret.map(crate::validator::Secret::from),
            ..crate::Settings::default()
        },
    )
    .unwrap();
    state.now = clock;
    state.guarded = dcl_http::Outbound::for_tests();
    if options.unbounded {
        state.limits = crate::limits::Limits::unbounded();
    }
    let state = Arc::new(state);
    let app = Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state.clone()));
    Node {
        pool,
        storage,
        app,
        state,
        oracle,
        slack,
    }
}

pub(super) async fn node() -> Option<Node> {
    set_clock(DateTime::from_timestamp_millis(Utc::now().timestamp_millis()).unwrap());
    Some(node_over(own_pool().await?, Options::default()).await)
}

pub(super) async fn send(app: &Router, request: Request<Body>) -> (u16, String) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 8 << 20)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

pub(super) fn json_of(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or(Value::Null)
}

/// A wallet's call, signed by `key` when given.
pub(super) async fn call(
    app: &Router,
    key: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = key {
        for (name, value) in signed_map(key, &method.to_lowercase(), path, "{}", false).await {
            request = request.header(name, value);
        }
    }
    let body = match body {
        Some(body) => {
            request = request.header("content-type", "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    let (status, body) = send(app, request.body(body).unwrap()).await;
    (status, json_of(&body))
}

/// How a validator call is signed.
#[derive(Clone)]
struct Signing {
    secret: &'static str,
    offset_ms: i64,
    method: Option<&'static str>,
    path: Option<String>,
    source: &'static str,
}

impl Default for Signing {
    fn default() -> Self {
        Self {
            secret: SECRET,
            offset_ms: 0,
            method: None,
            path: None,
            source: "203.0.113.7",
        }
    }
}

/// A validator call signed as `signing` says; without an offset, at the
/// node's clock or a millisecond after the last call's timestamp, so no two
/// are the same request.
fn validator_request(method: &str, path: &str, body: &str, signing: &Signing) -> Request<Body> {
    let now = clock().timestamp_millis();
    let sent = match signing.offset_ms {
        0 => SENT.with(|last| {
            let sent = if (0..60_000).contains(&(last.get() - now)) {
                last.get() + 1
            } else {
                now
            };
            last.set(sent);
            sent
        }),
        offset => now + offset,
    };
    let timestamp = sent.to_string();
    let signature = sign(
        signing.secret.as_bytes(),
        &timestamp,
        signing.method.unwrap_or(method),
        signing.path.as_deref().unwrap_or(path),
        body.as_bytes(),
    );
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("x-real-ip", signing.source)
        .header(TIMESTAMP_HEADER, timestamp)
        .header(SIGNATURE_HEADER, signature)
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub(super) async fn validator_call(
    app: &Router,
    method: &str,
    path: &str,
    body: &str,
) -> (u16, String) {
    send(
        app,
        validator_request(method, path, body, &Signing::default()),
    )
    .await
}

pub(super) async fn claim(node: &Node) -> (u16, Value) {
    let (status, body) = validator_call(&node.app, "POST", CLAIMS, "{}").await;
    (status, json_of(&body))
}

pub(super) async fn lease(node: &Node, validation: &Value, claim: &Value) -> (u16, Value) {
    let (status, body) = validator_call(
        &node.app,
        "POST",
        &format!("{CLAIMS}/{}/lease", validation.as_str().unwrap()),
        &json!({ "claimId": claim }).to_string(),
    )
    .await;
    (status, json_of(&body))
}

pub(super) async fn verdict(node: &Node, collection: Uuid, result: &Value) -> (u16, Value) {
    let (status, body) = validator_call(
        &node.app,
        "POST",
        &format!("/v1/collections/{collection}/validation-result"),
        &result.to_string(),
    )
    .await;
    (status, json_of(&body))
}

/// The verdict a claim's validator posts: every item it was sent, `passed`
/// as the verdict says.
pub(super) fn result_of(claimed: &Value, verdict: &str) -> Value {
    let data = &claimed["data"];
    json!({
        "validationId": data["validationId"],
        "collectionId": data["collectionId"],
        "verdict": verdict,
        "rulesVersion": "1.4.0+build.7",
        "items": data["items"].as_array().unwrap().iter().map(|item| json!({
            "itemId": item["itemId"],
            "contentHash": item["contentHash"],
            "passed": verdict == "passed",
            "findings": if verdict == "rejected" {
                json!([{ "rule": "triangles", "check": "count", "severity": "error", "message": "Too many triangles", "measured": 1.5e4, "limit": 10000 }])
            } else {
                json!([])
            },
        })).collect::<Vec<_>>(),
        "claimId": data["claimId"],
    })
}

/// A standard draft of the owner's named `name` with `items` items, each
/// naming a model file of its own (and the first a video) that the owner
/// uploaded.
pub(super) async fn draft(node: &Node, name: &str, items: usize) -> (Uuid, Vec<Uuid>) {
    let id = Uuid::new_v4();
    let owner = address(OWNER_KEY);
    save_collection(&node.state.items, &owner, json!({ "id": id, "name": name }))
        .await
        .unwrap();
    let mut ids = Vec::new();
    for n in 0..items {
        let item = Uuid::new_v4();
        let model = unique("validator model");
        let thumbnail = unique("validator thumbnail");
        let video = unique("validator video");
        let mut files: Vec<(&str, &[u8])> =
            vec![("model.glb", &model), ("thumbnail.png", &thumbnail)];
        if n == 0 {
            files.push(("video.mp4", &video));
        }
        saved_item(
            &node.state.items,
            &owner,
            &json!({ "id": item, "name": format!("Item {n}"), "collection_id": id, "rarity": "common", "contents": contents_of(&files) }),
        )
        .await;
        let bytes: Vec<&[u8]> = files.iter().map(|(_, bytes)| *bytes).collect();
        upload(&node.state.items, &owner, item, &bytes)
            .await
            .unwrap();
        ids.push(item);
        advance(Duration::milliseconds(1));
    }
    (id, ids)
}

/// A standard draft with `items` bare items, written directly.
async fn bare_draft(node: &Node, name: &str, items: usize) -> (Uuid, Vec<Uuid>) {
    let id = Uuid::new_v4();
    save_collection(
        &node.state.items,
        &address(OWNER_KEY),
        json!({ "id": id, "name": name }),
    )
    .await
    .unwrap();
    let mut ids = Vec::new();
    for n in 0..items {
        let item = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO builder.items (id, collection_id, eth_address, name, type, rarity, data, metrics, created_at) \
             VALUES ($1, $2, $3, $4, 'wearable', 'common', '{\"category\":\"hat\"}', '{}', \
               timezone('UTC', now()) + make_interval(secs => $5))",
        )
        .bind(item)
        .bind(id)
        .bind(address(OWNER_KEY))
        .bind(format!("Item {n}"))
        .bind(n as f64)
        .execute(&node.pool)
        .await
        .unwrap();
        ids.push(item);
    }
    (id, ids)
}

pub(super) async fn contract_of(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT contract_address FROM builder.collections WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// `POST /v1/collections/:id/publish` once the chain holds `on_chain` items.
pub(super) async fn publish(node: &Node, id: Uuid, on_chain: u64) -> (u16, Value) {
    node.oracle
        .deploy(&contract_of(&node.pool, id).await, on_chain);
    call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/publish"),
        None,
    )
    .await
}

pub(super) async fn published(node: &Node, name: &str, items: usize) -> (Uuid, Vec<Uuid>) {
    let (id, ids) = draft(node, name, items).await;
    let (status, body) = publish(node, id, items as u64).await;
    assert_eq!(status, 200, "{body}");
    (id, ids)
}

pub(super) type RequestRow = (Uuid, String, Vec<Uuid>, i16, Option<String>);

pub(super) async fn requests(pool: &PgPool, collection: Uuid) -> Vec<RequestRow> {
    sqlx::query_as(
        "SELECT id, trigger, item_ids, claims, outcome FROM builder.validation_requests \
         WHERE collection_id = $1 ORDER BY created_at, id",
    )
    .bind(collection)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn every_request(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM builder.validation_requests")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// What the curators and creators see of a collection's review: its
/// curations, item curations and timeline.
pub(super) async fn review_state(pool: &PgPool) -> String {
    sqlx::query_scalar(
        "SELECT md5(concat( \
           (SELECT string_agg(c::text, ',' ORDER BY id) FROM builder.collection_curations c), \
           (SELECT string_agg(i::text, ',' ORDER BY id) FROM builder.item_curations i), \
           (SELECT string_agg(e::text, ',' ORDER BY id) FROM builder.collection_events e)))",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

pub(super) async fn collection_curation(pool: &PgPool, collection: Uuid, status: &str) {
    sqlx::query(
        "INSERT INTO builder.collection_curations (id, collection_id, status, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(collection)
    .bind(status)
    .bind(clock().naive_utc())
    .execute(pool)
    .await
    .unwrap();
}

/// The chain copy holds `contract` with each `(on-chain id, content hash)`.
pub(super) async fn copy_holds(pool: &PgPool, contract: &str, items: &[(u64, &str)]) {
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
           is_completed, is_approved, is_editable, managers, minters, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'Validator', $2, $2, 'urn:validator', true, true, true, '{}', '{}', 1, 1, 1, timezone('UTC', now())) \
         ON CONFLICT DO NOTHING",
    )
    .bind(contract)
    .bind(address(OWNER_KEY))
    .execute(pool)
    .await
    .unwrap();
    for (item, hash) in items {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, \
               total_supply, max_supply, available, price, name, category, content_hash, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, $2::numeric, 137, 'wearable_v2', $3, $4, 'common', 0, 100000, 100000, 0, 'Item', 'hat', $5, \
               1, 1, 1, timezone('UTC', now())) \
             ON CONFLICT (network, contract_address, item_id) DO UPDATE SET content_hash = EXCLUDED.content_hash",
        )
        .bind(contract)
        .bind(item.to_string())
        .bind(format!("urn:decentraland:matic:collections-v2:{contract}:{item}"))
        .bind(address(OWNER_KEY))
        .bind(hash)
        .execute(pool)
        .await
        .unwrap();
    }
}

pub(super) async fn local_hash(pool: &PgPool, item: Uuid) -> String {
    sqlx::query_scalar("SELECT local_content_hash FROM builder.items WHERE id = $1")
        .bind(item)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The owner saves item `item` of `collection` again, as it is, and the
/// node computes its content hash.
pub(super) async fn resave(node: &Node, collection: Uuid, item: Uuid, n: usize, model: &[u8]) {
    let item_row: Value = super::support::read_item(&node.state.items, item)
        .await
        .unwrap();
    let mut files: Vec<(String, String)> = item_row["contents"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(file, hash)| (file.clone(), hash.as_str().unwrap().to_string()))
        .collect();
    for (file, hash) in &mut files {
        if file == "model.glb" {
            *hash = dcl_auth::hash_bytes_v1(model);
        }
    }
    let contents: serde_json::Map<String, Value> =
        files.into_iter().map(|(f, h)| (f, h.into())).collect();
    saved_item(
        &node.state.items,
        &address(OWNER_KEY),
        &json!({ "id": item, "name": format!("Item {n}"), "collection_id": collection, "rarity": "common", "contents": contents }),
    )
    .await;
    upload(&node.state.items, &address(OWNER_KEY), item, &[model])
        .await
        .unwrap();
}

pub(super) fn notice(collection: Uuid, name: &str, reason: &str) -> String {
    format!(
        "⚠️ No automated review for <https://creators.example.org/collections/editor?collection={collection}&reviewing=true|{name}>: {reason}. It needs a curator review without it."
    )
}

fn published_notice(collection: Uuid, name: &str, items: usize) -> String {
    let owner = address(OWNER_KEY);
    format!(
        "🎨 New collection ready for review: <https://creators.example.org/collections/editor?collection={collection}&reviewing=true|{name}> by <https://creators.example.org/shop/items/creator/{owner}|{}> ({items} {}).",
        crate::curation::slack::shorten(&owner),
        if items == 1 { "item" } else { "items" }
    )
}

pub(super) async fn sweep(node: &Node) {
    crate::validator::sweep(&node.state).await.unwrap();
    node.state.notices.settled().await;
}

#[tokio::test]
async fn a_first_publication_records_one_request_for_its_items() {
    let Some(node) = node().await else {
        return;
    };
    let (refused, _) = draft(&node, "Refused publication", 2).await;
    assert_eq!(
        publish(&node, refused, 1).await.0,
        409,
        "the chain lacks an item"
    );
    assert_eq!(
        every_request(&node.pool).await,
        0,
        "a refused publication records nothing"
    );
    let (id, items) = draft(&node, "First publication", 2).await;
    assert_eq!(publish(&node, id, 2).await.0, 200);
    let recorded = requests(&node.pool, id).await;
    assert_eq!(recorded.len(), 1, "{recorded:?}");
    assert_eq!(
        (
            recorded[0].1.as_str(),
            recorded[0].2.clone(),
            recorded[0].3,
            recorded[0].4.clone()
        ),
        ("publish", items.clone(), 0, None)
    );
    assert_eq!(publish(&node, id, 2).await.0, 200);
    assert_eq!(
        requests(&node.pool, id).await,
        recorded,
        "a repeated publish records nothing"
    );
    let (approved, _) = draft(&node, "Approved on chain", 1).await;
    copy_holds(
        &node.pool,
        &contract_of(&node.pool, approved).await,
        &[(0, "bafkreiold")],
    )
    .await;
    assert_eq!(publish(&node, approved, 1).await.0, 200);
    assert_eq!(
        requests(&node.pool, approved).await,
        vec![],
        "an approved collection is not reviewed"
    );
    node.state.notices.settled().await;
    assert_eq!(
        node.slack.texts(),
        vec![published_notice(id, "First publication", 2)]
    );
}

#[tokio::test]
async fn two_concurrent_first_publications_record_one_request() {
    let Some(node) = node().await else {
        return;
    };
    let (id, _) = draft(&node, "Raced", 2).await;
    node.oracle.deploy(&contract_of(&node.pool, id).await, 2);
    let path = format!("/v1/collections/{id}/publish");
    let mut held = node.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(id)
        .execute(&mut *held)
        .await
        .unwrap();
    let release = async {
        loop {
            let waiting: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND wait_event_type = 'Lock'",
            )
            .fetch_one(&node.pool)
            .await
            .unwrap();
            if waiting == 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        held.rollback().await.unwrap();
    };
    let (a, b, ()) = tokio::join!(
        call(&node.app, Some(OWNER_KEY), "POST", &path, None),
        call(&node.app, Some(OWNER_KEY), "POST", &path, None),
        release
    );
    assert_eq!((a.0, b.0), (200, 200));
    assert_eq!(
        every_request(&node.pool).await,
        1,
        "both read the draft before either published"
    );
    node.state.notices.settled().await;
    assert_eq!(
        node.slack.texts(),
        vec![published_notice(id, "Raced", 2)],
        "one notice"
    );
}

#[tokio::test]
async fn a_changes_submission_requests_the_items_whose_content_differs_from_the_chain() {
    let Some(node) = node().await else {
        return;
    };
    let (id, items) = published(&node, "Changes", 3).await;
    let first = requests(&node.pool, id).await;
    let contract = contract_of(&node.pool, id).await;
    let new_model = unique("changed model");
    for (n, item) in items.iter().enumerate() {
        resave(&node, id, *item, n, &unique("same model")).await;
    }
    resave(&node, id, items[1], 1, &new_model).await;
    let (h0, h2) = (
        local_hash(&node.pool, items[0]).await,
        local_hash(&node.pool, items[2]).await,
    );
    copy_holds(
        &node.pool,
        &contract,
        &[(0, &h0), (1, "bafkreiold"), (2, &h2)],
    )
    .await;
    collection_curation(&node.pool, id, "approved").await;
    advance(Duration::minutes(1));
    let path = format!("/v1/collections/{id}/curation");
    let (status, body) = call(&node.app, Some(OWNER_KEY), "POST", &path, Some(json!({}))).await;
    assert_eq!(status, 200, "{body}");
    let now = requests(&node.pool, id).await;
    assert_eq!(now.len(), 2, "{now:?}");
    assert_eq!(
        (now[0].0, now[0].4.as_deref()),
        (first[0].0, Some("superseded")),
        "the newer request supersedes the open one"
    );
    assert_eq!(
        (now[1].1.as_str(), now[1].2.clone(), now[1].4.clone()),
        ("changes", vec![items[1]], None)
    );
    let h1 = local_hash(&node.pool, items[1]).await;
    copy_holds(&node.pool, &contract, &[(1, &h1)]).await;
    collection_curation(&node.pool, id, "approved").await;
    advance(Duration::minutes(1));
    assert_eq!(
        call(&node.app, Some(OWNER_KEY), "POST", &path, Some(json!({})))
            .await
            .0,
        200
    );
    assert_eq!(
        requests(&node.pool, id).await,
        now,
        "nothing differs: no request"
    );
}

#[tokio::test]
async fn a_collection_over_fifty_items_gets_no_request_and_the_curators_hear_of_it() {
    let Some(node) = node().await else {
        return;
    };
    let (fifty, items) = bare_draft(&node, "Fifty", 50).await;
    assert_eq!(publish(&node, fifty, 50).await.0, 200);
    assert_eq!(requests(&node.pool, fifty).await[0].2, items);
    let (large, _) = bare_draft(&node, "Large", 51).await;
    assert_eq!(publish(&node, large, 51).await.0, 200);
    assert_eq!(requests(&node.pool, large).await, vec![]);
    node.state.notices.settled().await;
    let mut expected = vec![
        published_notice(fifty, "Fifty", 50),
        published_notice(large, "Large", 51),
        notice(
            large,
            "Large",
            "it is too large for the automated review (51 items)",
        ),
    ];
    expected.sort();
    assert_eq!(node.slack.texts(), expected);
}

#[tokio::test]
async fn the_sixth_request_of_a_utc_day_for_one_collection_is_not_recorded() {
    let Some(node) = node().await else {
        return;
    };
    set_clock(at("2026-05-04T10:00:00.000Z"));
    let (id, items) = draft(&node, "Busy", 1).await;
    for minute in 0..5 {
        sqlx::query(
            "INSERT INTO builder.validation_requests (id, collection_id, trigger, item_ids, created_at, settled_at, outcome) \
             VALUES ($1, $2, 'changes', $3, $4, $4, 'superseded')",
        )
        .bind(Uuid::new_v4())
        .bind(id)
        .bind(&items)
        .bind((at("2026-05-04T23:00:00.000Z") + Duration::minutes(minute)).naive_utc())
        .execute(&node.pool)
        .await
        .unwrap();
    }
    assert_eq!(publish(&node, id, 1).await.0, 200);
    assert_eq!(every_request(&node.pool).await, 5, "the sixth of the day");
    set_clock(at("2026-05-05T00:30:00.000Z"));
    resave(&node, id, items[0], 0, &unique("next day model")).await;
    copy_holds(
        &node.pool,
        &contract_of(&node.pool, id).await,
        &[(0, "bafkreiold")],
    )
    .await;
    collection_curation(&node.pool, id, "approved").await;
    advance(Duration::minutes(1));
    let (status, body) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/curation"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let now = requests(&node.pool, id).await;
    assert_eq!(
        (now.len(), now[5].1.as_str(), now[5].4.clone()),
        (6, "changes", None),
        "a new UTC day takes requests again"
    );
}

#[tokio::test]
async fn without_the_secret_or_the_flag_nothing_is_recorded_and_the_routes_refuse_alike() {
    set_clock(at("2026-05-04T10:00:00.000Z"));
    let Some(pool) = own_pool().await else {
        return;
    };
    let enabled = node_over(pool.clone(), Options::default()).await;
    let unset = node_over(
        pool.clone(),
        Options {
            secret: None,
            ..Options::default()
        },
    )
    .await;
    let forged = send(
        &enabled.app,
        validator_request(
            "POST",
            CLAIMS,
            "{}",
            &Signing {
                secret: OTHER_SECRET,
                ..Signing::default()
            },
        ),
    )
    .await;
    assert_eq!(forged, (401, UNAUTHORIZED.to_string()));
    for (label, node, document, local) in [
        ("the secret unset", &unset, Some(json!(true)), None),
        ("no flags document", &enabled, None, None),
        (
            "an operator turns the flag off",
            &enabled,
            Some(json!(false)),
            None,
        ),
        ("the flag a string", &enabled, Some(json!("true")), None),
        ("the flag null", &enabled, Some(Value::Null), None),
        (
            "the flag overridden off",
            &enabled,
            Some(json!(true)),
            Some(false),
        ),
    ] {
        set_flag(&pool, document).await;
        set_local_flag(&pool, local).await;
        let (id, _) = published(node, label, 1).await;
        assert_eq!(requests(&pool, id).await, vec![], "{label}");
        assert_eq!(
            validator_call(&node.app, "POST", CLAIMS, "{}").await,
            forged,
            "{label}: refused as a forgery is"
        );
    }
    set_flag(&pool, Some(json!(false))).await;
    set_local_flag(&pool, Some(true)).await;
    let (id, _) = published(&enabled, "Overridden on", 1).await;
    assert_eq!(
        requests(&pool, id).await.len(),
        1,
        "the node's override turns it on"
    );
    assert_eq!(claim(&enabled).await.0, 200);
}

#[tokio::test]
async fn the_validator_claims_the_oldest_request_with_its_manifest_and_reads_its_files_unsigned() {
    let Some(node) = node().await else {
        return;
    };
    let (older, items) = published(&node, "Older", 2).await;
    advance(Duration::minutes(1));
    let (newer, _) = published(&node, "Newer", 1).await;
    let replaced: String = sqlx::query_scalar(
        "SELECT hash FROM builder.item_contents WHERE item_id = $1 AND file = 'model.glb'",
    )
    .bind(items[1])
    .fetch_one(&node.pool)
    .await
    .unwrap();
    let new_model = unique("edited after the request");
    resave(&node, older, items[1], 1, &new_model).await;
    let (status, claimed) = claim(&node).await;
    assert_eq!(status, 200, "{claimed}");
    let data = &claimed["data"];
    let request = requests(&node.pool, older).await;
    assert_eq!(data["validationId"], json!(request[0].0));
    assert_eq!(data["collectionId"], json!(older));
    assert!(Uuid::parse_str(data["claimId"].as_str().unwrap()).is_ok());
    assert_eq!(
        data["leaseUntil"],
        json!(iso(clock() + Duration::minutes(30)))
    );
    let contract = contract_of(&node.pool, older).await;
    let sent: Vec<Value> = data["items"].as_array().unwrap().clone();
    assert_eq!(
        sent.iter()
            .map(|item| item["itemId"].clone())
            .collect::<Vec<_>>(),
        vec![json!(items[0]), json!(items[1])]
    );
    for (n, item) in sent.iter().enumerate() {
        let contents = item["contents"].as_object().unwrap();
        assert_eq!(
            contents.keys().collect::<Vec<_>>(),
            vec!["model.glb", "thumbnail.png"],
            "the video is not sent"
        );
        let metadata = &item["metadata"];
        assert_eq!(
            metadata,
            &json!({
                "id": format!("urn:decentraland:matic:collections-v2:{contract}:{n}"),
                "name": format!("Item {n}"),
                "description": "",
                "collectionAddress": contract,
                "rarity": "common",
                "i18n": [{ "code": "en", "text": format!("Item {n}") }],
                "data": {
                    "replaces": [], "hides": [], "tags": [], "category": "hat",
                    "representations": [{
                        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
                        "mainFile": "model.glb", "contents": ["model.glb"], "overrideHides": [], "overrideReplaces": []
                    }]
                },
                "image": "image.png",
                "thumbnail": "thumbnail.png",
                "metrics": { "meshes": 1, "bodies": 1, "materials": 1, "textures": 1, "triangles": 1, "entities": 1 },
            })
        );
        for hash in contents.values() {
            let (status, _) = call(
                &node.app,
                None,
                "GET",
                &format!("/v1/storage/contents/{}", hash.as_str().unwrap()),
                None,
            )
            .await;
            assert_eq!(status, 200, "{hash} opens to anyone");
        }
    }
    assert_eq!(
        sent[1]["contents"]["model.glb"],
        json!(dcl_auth::hash_bytes_v1(&new_model)),
        "the item as saved now"
    );
    assert_eq!(
        sent[1]["contentHash"],
        json!(local_hash(&node.pool, items[1]).await)
    );
    assert_eq!(
        call(
            &node.app,
            None,
            "GET",
            &format!("/v1/storage/contents/{replaced}"),
            None
        )
        .await
        .0,
        404,
        "a replaced file stays private"
    );
    let raw = validator_call(&node.app, "POST", CLAIMS, "{}").await;
    assert_eq!(raw.0, 200);
    let order = raw.1.find(r#""metadata":{"id":"#).is_some()
        && raw.1.contains(r#""i18n":[{"code":"en","text":"Item 0"}],"data":{"replaces":[],"hides":[],"tags":[],"category":"hat","representations""#);
    assert!(
        order,
        "the metadata keeps JSON.stringify's order: {}",
        raw.1
    );
    assert_eq!(json_of(&raw.1)["data"]["collectionId"], json!(newer));
    let stored: Value = sqlx::query_scalar(
        "SELECT content_hashes FROM builder.validation_requests WHERE collection_id = $1",
    )
    .bind(older)
    .fetch_one(&node.pool)
    .await
    .unwrap();
    assert_eq!(
        stored,
        sent.iter()
            .map(|item| (
                item["itemId"].as_str().unwrap().to_string(),
                item["contentHash"].clone()
            ))
            .collect::<serde_json::Map<_, _>>()
            .into_iter()
            .collect::<Value>()
    );
    assert_eq!(
        validator_call(&node.app, "POST", CLAIMS, "{}").await,
        (204, String::new())
    );
}

#[tokio::test]
async fn two_validators_claiming_at_once_never_get_the_same_request() {
    let Some(node) = node().await else {
        return;
    };
    let (a, _) = published(&node, "A", 1).await;
    advance(Duration::minutes(1));
    let (b, _) = published(&node, "B", 1).await;
    let mut held = node.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM builder.validation_requests WHERE collection_id = $1 FOR UPDATE")
        .bind(a)
        .execute(&mut *held)
        .await
        .unwrap();
    let (status, claimed) = tokio::time::timeout(std::time::Duration::from_secs(10), claim(&node))
        .await
        .expect("a claim never waits on another's request");
    assert_eq!(
        (status, claimed["data"]["collectionId"].clone()),
        (200, json!(b))
    );
    held.rollback().await.unwrap();
    let (first, second) = tokio::join!(claim(&node), claim(&node));
    let mut statuses = [first.0, second.0];
    statuses.sort();
    assert_eq!(statuses, [200, 204], "{first:?} {second:?}");
    assert_eq!(requests(&node.pool, a).await[0].3, 1, "claimed once");
}

#[tokio::test]
async fn the_validator_extends_its_lease_for_two_hours_at_most_and_only_for_its_current_claim() {
    let Some(node) = node().await else {
        return;
    };
    let (_, _) = published(&node, "Leased", 1).await;
    let start = clock();
    let (_, claimed) = claim(&node).await;
    let (validation, claim_id) = (
        claimed["data"]["validationId"].clone(),
        claimed["data"]["claimId"].clone(),
    );
    for (minutes, until) in [(20, 50), (45, 75), (70, 100), (95, 120), (119, 120)] {
        set_clock(start + Duration::minutes(minutes));
        assert_eq!(
            lease(&node, &validation, &claim_id).await,
            (
                200,
                json!({ "ok": true, "data": { "leaseUntil": iso(start + Duration::minutes(until)) } })
            ),
            "at {minutes} minutes"
        );
    }
    let stale = json!({ "ok": false, "data": {}, "error": "The claim is not current" });
    for (label, validation, claim_id) in [
        (
            "another claim id",
            validation.clone(),
            json!(Uuid::new_v4()),
        ),
        (
            "an unknown request",
            json!(Uuid::new_v4()),
            claim_id.clone(),
        ),
    ] {
        assert_eq!(
            lease(&node, &validation, &claim_id).await,
            (409, stale.clone()),
            "{label}"
        );
    }
    set_clock(start + Duration::minutes(120));
    assert_eq!(
        lease(&node, &validation, &claim_id).await,
        (409, stale.clone()),
        "two hours after the claim"
    );
    set_clock(start + Duration::minutes(120) + Duration::milliseconds(1));
    let (status, again) = claim(&node).await;
    assert_eq!(
        (status, again["data"]["validationId"].clone()),
        (200, validation.clone())
    );
    assert_eq!(
        lease(&node, &validation, &claim_id).await,
        (409, stale.clone()),
        "a claim taken over"
    );
    advance(Duration::minutes(30) + Duration::milliseconds(1));
    assert_eq!(
        lease(&node, &validation, &again["data"]["claimId"]).await,
        (409, stale),
        "a lapsed lease"
    );
}

#[tokio::test]
async fn a_verdict_from_the_current_claim_settles_the_request_and_changes_no_curation() {
    let Some(node) = node().await else {
        return;
    };
    let (id, items) = published(&node, "Judged", 2).await;
    collection_curation(&node.pool, id, "pending").await;
    let before = review_state(&node.pool).await;
    let (_, claimed) = claim(&node).await;
    let result = result_of(&claimed, "rejected");
    assert_eq!(verdict(&node, id, &result).await, (204, Value::Null));
    let settled = requests(&node.pool, id).await;
    assert_eq!(settled[0].4.as_deref(), Some("rejected"));
    advance(Duration::seconds(1));
    let mut passed = result.clone();
    passed["verdict"] = json!("passed");
    assert_eq!(
        verdict(&node, id, &passed).await,
        (204, Value::Null),
        "a second verdict"
    );
    assert_eq!(requests(&node.pool, id).await, settled, "counts once");
    assert_eq!(
        review_state(&node.pool).await,
        before,
        "no curation, item curation or event"
    );
    node.state.notices.settled().await;
    assert_eq!(
        node.slack.texts(),
        vec![published_notice(id, "Judged", 2)],
        "no notice"
    );

    let (other, _) = published(&node, "Other", 1).await;
    let (_, other_claim) = claim(&node).await;
    let other_result = result_of(&other_claim, "passed");
    let mut misplaced = other_result.clone();
    misplaced["collectionId"] = json!(id);
    assert_eq!(
        verdict(&node, id, &misplaced).await,
        (
            400,
            json!({ "ok": false, "data": { "id": id }, "error": "The result belongs to another collection" })
        ),
        "a request of another collection"
    );
    assert_eq!(
        verdict(&node, id, &other_result).await,
        (
            400,
            json!({ "ok": false, "data": { "id": id, "collectionId": other }, "error": "The result belongs to another collection" })
        ),
        "the body names another collection than the path"
    );
    let mut foreign = other_result.clone();
    foreign["items"][0]["itemId"] = json!(items[0]);
    assert_eq!(
        verdict(&node, other, &foreign).await.0,
        400,
        "an item the request didn't name"
    );
    let mut wrong_claim = other_result.clone();
    wrong_claim["claimId"] = json!(Uuid::new_v4());
    let mut unknown = other_result.clone();
    unknown["validationId"] = json!(Uuid::new_v4());
    for (label, body) in [
        ("another claim id", &wrong_claim),
        ("an unknown request", &unknown),
    ] {
        assert_eq!(
            verdict(&node, other, body).await,
            (204, Value::Null),
            "{label}"
        );
        assert_eq!(
            requests(&node.pool, other).await[0].4,
            None,
            "{label} settles nothing"
        );
    }
    advance(Duration::minutes(30) + Duration::milliseconds(1));
    assert_eq!(
        verdict(&node, other, &other_result).await,
        (204, Value::Null),
        "a lapsed claim"
    );
    assert_eq!(
        requests(&node.pool, other).await[0].4,
        None,
        "a lapsed claim settles nothing"
    );
    let (_, retaken) = claim(&node).await;
    assert_eq!(
        verdict(&node, other, &other_result).await,
        (204, Value::Null),
        "a claim taken over"
    );
    assert_eq!(requests(&node.pool, other).await[0].4, None);
    let mut retryable = result_of(&retaken, "error");
    retryable["retryable"] = json!(true);
    retryable["reason"] = json!("render server crashed");
    assert_eq!(verdict(&node, other, &retryable).await, (204, Value::Null));
    let (status, third) = claim(&node).await;
    assert_eq!(
        (
            status,
            third["data"]["validationId"].clone(),
            requests(&node.pool, other).await[0].3
        ),
        (200, retaken["data"]["validationId"].clone(), 3),
        "a retryable error frees the request at once"
    );
    assert_eq!(
        verdict(&node, other, &result_of(&third, "passed")).await,
        (204, Value::Null)
    );
    assert_eq!(
        requests(&node.pool, other).await[0].4.as_deref(),
        Some("passed")
    );
}

/// A verdict names its request and its items by the ids the claim gave, in
/// that one spelling: any other form of the same id is a bad body and
/// settles nothing.
#[tokio::test]
async fn a_verdict_naming_its_request_in_another_spelling_is_refused_and_settles_nothing() {
    let Some(node) = node().await else {
        return;
    };
    let (id, _) = published(&node, "Spelled", 1).await;
    let (_, claimed) = claim(&node).await;
    let result = result_of(&claimed, "rejected");
    let canonical = result["validationId"].as_str().unwrap().to_string();
    let simple = canonical.replace('-', "");
    for spelling in [
        canonical.to_uppercase(),
        simple,
        format!("urn:uuid:{canonical}"),
        format!("{{{canonical}}}"),
    ] {
        let mut body = result.clone();
        body["validationId"] = json!(spelling);
        assert_eq!(
            verdict(&node, id, &body).await,
            (
                400,
                json!({ "ok": false, "data": {}, "error": "Invalid request body" })
            ),
            "{spelling}"
        );
        assert_eq!(requests(&node.pool, id).await[0].4, None, "{spelling}");
    }
    let item = result["items"][0]["itemId"].as_str().unwrap().to_string();
    for spelling in [
        item.to_uppercase(),
        item.replace('-', ""),
        format!("urn:uuid:{item}"),
    ] {
        let mut body = result.clone();
        body["items"][0]["itemId"] = json!(spelling);
        assert_eq!(
            verdict(&node, id, &body).await,
            (
                400,
                json!({ "ok": false, "data": {}, "error": "Invalid request body" })
            ),
            "{spelling}"
        );
        assert_eq!(requests(&node.pool, id).await[0].4, None, "{spelling}");
    }
    assert_eq!(verdict(&node, id, &result).await, (204, Value::Null));
    assert_eq!(
        requests(&node.pool, id).await[0].4.as_deref(),
        Some("rejected")
    );
}

#[tokio::test]
async fn a_validator_that_dies_lets_its_lease_lapse_and_the_third_lapse_abandons_with_one_notice() {
    let Some(node) = node().await else {
        return;
    };
    let (id, _) = published(&node, "Crashing", 1).await;
    let mut claim_ids = Vec::new();
    for _ in 0..3 {
        let (status, claimed) = claim(&node).await;
        assert_eq!(status, 200, "{claimed}");
        claim_ids.push(claimed["data"]["claimId"].clone());
        advance(Duration::minutes(30));
        assert_eq!(
            claim(&node).await.0,
            204,
            "the lease holds to its last instant"
        );
        advance(Duration::milliseconds(1));
    }
    claim_ids.dedup();
    assert_eq!(claim_ids.len(), 3, "each claim has its own id");
    assert_eq!(claim(&node).await.0, 204, "three claims at most");
    assert_eq!(requests(&node.pool, id).await[0].4, None);
    sweep(&node).await;
    sweep(&node).await;
    assert_eq!(
        requests(&node.pool, id).await[0].4.as_deref(),
        Some("abandoned")
    );
    let timeline: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM builder.collection_events WHERE collection_id = $1",
    )
    .bind(id)
    .fetch_one(&node.pool)
    .await
    .unwrap();
    assert_eq!(
        timeline, 0,
        "without the review flow the timeline records nothing"
    );
    assert_eq!(node.slack.texts(), {
        let mut expected = vec![
            published_notice(id, "Crashing", 1),
            notice(
                id,
                "Crashing",
                "the automated review got no verdict after 3 attempts",
            ),
        ];
        expected.sort();
        expected
    });
}

#[tokio::test]
async fn unclaimed_requests_expire_after_seven_days_and_settled_ones_are_deleted_after_thirty() {
    let Some(node) = node().await else {
        return;
    };
    let created = clock();
    let (id, _) = published(&node, "Forgotten", 1).await;
    set_clock(created + Duration::days(7) - Duration::milliseconds(1));
    sweep(&node).await;
    assert_eq!(requests(&node.pool, id).await[0].4, None);
    set_clock(created + Duration::days(7));
    let (second, _) = published(&node, "Raced sweeps", 1).await;
    set_clock(created + Duration::days(7) + Duration::milliseconds(1));
    tokio::join!(sweep(&node), sweep(&node));
    sweep(&node).await;
    assert_eq!(
        requests(&node.pool, id).await[0].4.as_deref(),
        Some("expired")
    );
    assert_eq!(requests(&node.pool, second).await[0].4, None);
    let expired = notice(id, "Forgotten", "the automated review did not run");
    assert_eq!(
        node.slack
            .texts()
            .iter()
            .filter(|text| **text == expired)
            .count(),
        1,
        "one notice from two passes"
    );
    let settled = clock();
    set_clock(settled + Duration::days(30) - Duration::milliseconds(1));
    sweep(&node).await;
    assert_eq!(requests(&node.pool, id).await.len(), 1);
    set_clock(settled + Duration::days(30));
    sweep(&node).await;
    assert_eq!(
        requests(&node.pool, id).await,
        vec![],
        "deleted thirty days after it settled"
    );
    assert_eq!(requests(&node.pool, second).await.len(), 1);
}

#[tokio::test]
async fn every_forgery_fails_before_anything_is_read_or_written() {
    set_clock(at("2026-05-04T10:00:00.000Z"));
    let Some(unreachable) = crate::db::scratch::pool(1).await.map(|_| {
        sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(200))
            .connect_lazy("postgres://postgres@127.0.0.1:1/postgres")
            .unwrap()
    }) else {
        return;
    };
    let node = node_over(unreachable, Options::default()).await;
    let collection = Uuid::new_v4();
    let verdict_path = format!("/v1/collections/{collection}/validation-result");
    let lease_path = format!("{CLAIMS}/{}/lease", Uuid::new_v4());
    let body = json!({ "claimId": Uuid::new_v4() }).to_string();
    let routes = [
        ("POST", CLAIMS.to_string(), "{}".to_string()),
        ("POST", lease_path.clone(), body.clone()),
        ("POST", verdict_path.clone(), "{}".to_string()),
    ];
    for (method, path, body) in &routes {
        let unsigned = Request::builder()
            .method(*method)
            .uri(path)
            .body(Body::from(body.clone()))
            .unwrap();
        let mut forgeries = vec![("unsigned", unsigned)];
        for (label, signing) in [
            (
                "a wrong secret",
                Signing {
                    secret: OTHER_SECRET,
                    ..Signing::default()
                },
            ),
            (
                "a stale timestamp",
                Signing {
                    offset_ms: -300_001,
                    ..Signing::default()
                },
            ),
            (
                "a future timestamp",
                Signing {
                    offset_ms: 300_001,
                    ..Signing::default()
                },
            ),
            (
                "another route",
                Signing {
                    path: Some("/v1/validator/claims/other".into()),
                    ..Signing::default()
                },
            ),
            (
                "another method",
                Signing {
                    method: Some("PUT"),
                    ..Signing::default()
                },
            ),
            (
                "the other mount",
                Signing {
                    path: Some(format!("/builder-api{path}")),
                    ..Signing::default()
                },
            ),
        ] {
            forgeries.push((label, validator_request(method, path, body, &signing)));
        }
        let mut changed = validator_request(method, path, body, &Signing::default());
        *changed.body_mut() = Body::from(format!("{body} "));
        forgeries.push(("a body changed after signing", changed));
        let mut words = validator_request(method, path, body, &Signing::default());
        words
            .headers_mut()
            .insert(TIMESTAMP_HEADER, "now".parse().unwrap());
        forgeries.push(("a timestamp that isn't a number", words));
        for (label, request) in forgeries {
            assert_eq!(
                send(&node.app, request).await,
                (401, UNAUTHORIZED.to_string()),
                "{path}: {label}"
            );
        }
    }
    let other = format!("/v1/collections/{}/validation-result", Uuid::new_v4());
    assert_eq!(
        send(
            &node.app,
            validator_request(
                "POST",
                &other,
                "{}",
                &Signing {
                    secret: OTHER_SECRET,
                    ..Signing::default()
                }
            )
        )
        .await,
        send(
            &node.app,
            validator_request(
                "POST",
                &verdict_path,
                "{}",
                &Signing {
                    secret: OTHER_SECRET,
                    ..Signing::default()
                }
            )
        )
        .await,
        "an unknown collection answers as any forgery"
    );
    for (path, cap) in [
        (CLAIMS.to_string(), 4 << 10),
        (lease_path, 4 << 10),
        (verdict_path, 5 << 20),
    ] {
        let oversized = format!("\"{}\"", "a".repeat(cap - 1));
        let (status, _) = send(
            &node.app,
            validator_request("POST", &path, &oversized, &Signing::default()),
        )
        .await;
        assert_eq!(status, 413, "{path} over {cap} bytes");
    }
    let limited = node_over(node.pool.clone(), Options::default()).await;
    let forged = Signing {
        secret: OTHER_SECRET,
        source: "198.51.100.9",
        ..Signing::default()
    };
    for n in 0..60 {
        assert_eq!(
            send(
                &limited.app,
                validator_request("POST", CLAIMS, "{}", &forged)
            )
            .await
            .0,
            401,
            "call {n}"
        );
    }
    assert_eq!(
        send(
            &limited.app,
            validator_request("POST", CLAIMS, "{}", &forged)
        )
        .await,
        (
            429,
            r#"{"data":{},"error":"Too many requests, try again in a minute","ok":false}"#
                .to_string()
        )
    );
    assert_eq!(
        send(
            &limited.app,
            validator_request(
                "POST",
                CLAIMS,
                "{}",
                &Signing {
                    source: "198.51.100.10",
                    ..forged
                }
            )
        )
        .await
        .0,
        401,
        "another source has its own budget"
    );
    let internal = Signing {
        secret: OTHER_SECRET,
        source: "10.1.2.3",
        ..Signing::default()
    };
    for n in 0..=60 {
        assert_eq!(
            send(
                &limited.app,
                validator_request("POST", CLAIMS, "{}", &internal)
            )
            .await
            .0,
            401,
            "a call that names no single client spends no shared budget: call {n}"
        );
    }
}

#[tokio::test]
async fn a_signed_call_replayed_inside_its_window_is_refused_and_the_seen_signatures_are_swept() {
    let Some(node) = node().await else {
        return;
    };
    published(&node, "Replayed", 1).await;
    let first = validator_request("POST", CLAIMS, "{}", &Signing::default());
    let headers = first.headers().clone();
    let request = || {
        let mut again = Request::builder()
            .method("POST")
            .uri(CLAIMS)
            .body(Body::from("{}"))
            .unwrap();
        *again.headers_mut() = headers.clone();
        again
    };
    let signature = headers[SIGNATURE_HEADER].to_str().unwrap().to_string();
    assert_eq!(send(&node.app, first).await.0, 200);
    assert_eq!(
        send(&node.app, request()).await,
        (401, UNAUTHORIZED.to_string()),
        "the same request again"
    );
    let mut recased = request();
    recased.headers_mut().insert(
        SIGNATURE_HEADER,
        format!(
            "sha256={}",
            signature["sha256=".len()..].to_ascii_uppercase()
        )
        .parse()
        .unwrap(),
    );
    assert_eq!(
        send(&node.app, recased).await,
        (401, UNAUTHORIZED.to_string()),
        "its signature re-cased"
    );
    let seen = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM builder.validator_seen_signatures")
            .fetch_one(&node.pool)
            .await
            .unwrap()
    };
    assert_eq!(seen().await, 1);
    advance(Duration::minutes(5));
    sweep(&node).await;
    assert_eq!(seen().await, 1, "kept while its window lasts");
    advance(Duration::milliseconds(1));
    sweep(&node).await;
    assert_eq!(seen().await, 0, "dropped once its window has passed");
}

#[tokio::test]
async fn with_creator_writes_off_the_validator_routes_answer_503_and_reads_still_work() {
    let Some(on) = node().await else {
        return;
    };
    let (id, items) = published(&on, "Paused", 1).await;
    let off = node_over(
        on.pool.clone(),
        Options {
            writes: false,
            storage: Some(on.storage.clone()),
            ..Options::default()
        },
    )
    .await;
    let disabled =
        json!({ "ok": false, "data": {}, "error": crate::writes::DISABLED_MESSAGE }).to_string();
    for (path, body) in [
        (CLAIMS.to_string(), "{}".to_string()),
        (
            format!("{CLAIMS}/{}/lease", Uuid::new_v4()),
            json!({ "claimId": Uuid::new_v4() }).to_string(),
        ),
        (
            format!("/v1/collections/{id}/validation-result"),
            "{}".to_string(),
        ),
    ] {
        let response = off
            .app
            .clone()
            .oneshot(validator_request("POST", &path, &body, &Signing::default()))
            .await
            .unwrap();
        assert_eq!(
            (
                response.status().as_u16(),
                response
                    .headers()
                    .get("retry-after")
                    .map(|v| v.to_str().unwrap().to_string())
            ),
            (503, Some("3600".to_string())),
            "{path}"
        );
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 16)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&bytes), disabled);
    }
    assert_eq!(requests(&on.pool, id).await[0].3, 0, "nothing claimed");
    let model: String = sqlx::query_scalar(
        "SELECT hash FROM builder.item_contents WHERE item_id = $1 AND file = 'model.glb'",
    )
    .bind(items[0])
    .fetch_one(&on.pool)
    .await
    .unwrap();
    assert_eq!(
        call(
            &off.app,
            None,
            "GET",
            &format!("/v1/storage/contents/{model}"),
            None
        )
        .await
        .0,
        200
    );
    advance(Duration::days(8));
    sweep(&off).await;
    assert_eq!(
        requests(&on.pool, id).await[0].4,
        None,
        "the job writes nothing either"
    );
}

#[tokio::test]
async fn a_deleted_collection_takes_its_requests_and_a_locked_one_is_not_claimed() {
    let Some(node) = node().await else {
        return;
    };
    let (id, _) = published(&node, "Locked", 1).await;
    let (status, _) = call(
        &node.app,
        Some(OWNER_KEY),
        "POST",
        &format!("/v1/collections/{id}/lock"),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(claim(&node).await.0, 204, "locked for a day");
    advance(Duration::days(1));
    let (status, claimed) = claim(&node).await;
    assert_eq!(
        (status, claimed["data"]["collectionId"].clone()),
        (200, json!(id))
    );
    sqlx::query("DELETE FROM builder.collections WHERE id = $1")
        .bind(id)
        .execute(&node.pool)
        .await
        .unwrap();
    assert_eq!(every_request(&node.pool).await, 0);
    assert_eq!(
        verdict(&node, id, &result_of(&claimed, "passed")).await,
        (204, Value::Null)
    );
    assert_eq!(every_request(&node.pool).await, 0);
}

#[tokio::test]
async fn what_a_creator_names_reaches_the_validator_as_data_and_slack_escaped() {
    let Some(node) = node().await else {
        return;
    };
    let hostile = "<!channel> *Hats*\n✅ <https://evil.example|forged>";
    let (large, _) = bare_draft(&node, "Hostile", 51).await;
    sqlx::query("UPDATE builder.collections SET name = $2 WHERE id = $1")
        .bind(large)
        .bind(hostile)
        .execute(&node.pool)
        .await
        .unwrap();
    assert_eq!(publish(&node, large, 51).await.0, 200);
    node.state.notices.settled().await;
    let texts = node.slack.texts();
    let warning = texts
        .iter()
        .find(|t| t.starts_with("⚠️"))
        .expect("the notice");
    assert!(
        warning.contains(
            "|&lt;!channel&gt; \u{200b}*\u{200b}Hats\u{200b}*\u{200b} ✅ &lt;https:\u{200b}//evil.\u{200b}example|forged&gt;>: it is too large"
        ),
        "{warning}"
    );
    let (id, items) = draft(&node, "Data", 1).await;
    let name = "Hat \"quoted\"\n</script>\u{2028}";
    sqlx::query("UPDATE builder.items SET name = $2, description = $2 WHERE id = $1")
        .bind(items[0])
        .bind(name)
        .execute(&node.pool)
        .await
        .unwrap();
    assert_eq!(publish(&node, id, 1).await.0, 200);
    let (_, claimed) = claim(&node).await;
    let metadata = &claimed["data"]["items"][0]["metadata"];
    assert_eq!(
        (
            metadata["name"].clone(),
            metadata["description"].clone(),
            metadata["i18n"][0]["text"].clone()
        ),
        (json!(name), json!(name), json!(name))
    );
}
