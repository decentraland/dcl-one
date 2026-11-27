//! `POST /v1/items/:id/videos` as a creator, a stranger and an attacker meet
//! it, through the router `serve` mounts, with real signatures, the scratch
//! database and a blob store in a temporary directory: who may make the node
//! read a body, what is kept, what is refused before or while the body
//! streams, and how a video then reads back.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{Request, header};
use futures::StreamExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{
    FakeCommittee, OWNER_KEY, committee_index, on_chain, save_collection, saved_item, signed_map,
    temp_storage, unique, wallet,
};
use crate::items::video::{self, VIDEO_FILE};
use crate::storage::Storage;

const STRANGER_KEY: &str = "0x8101010101010101010101010101010101010101010101010101010101010101";
const COMMITTEE_KEY: &str = "0x8202020202020202020202020202020202020202020202020202020202020202";
const MANAGER_KEY: &str = "0x8303030303030303030303030303030303030303030303030303030303030303";
const OTHER_KEY: &str = "0x8404040404040404040404040404040404040404040404040404040404040404";
const BOUNDARY: &str = "videoboundary";

fn address(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

/// A playable MP4 container's top-level boxes around `label`: `ftyp`, a
/// `moov` and an `mdat` carrying the label, so every video hashes apart.
fn mp4(label: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut boxed = |kind: &[u8; 4], payload: &[u8]| {
        bytes.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
        bytes.extend_from_slice(kind);
        bytes.extend_from_slice(payload);
    };
    boxed(b"ftyp", b"isom\0\0\x02\0isomiso2mp41");
    boxed(b"moov", b"\0\0\0\x08mvhd");
    boxed(b"mdat", &unique(label));
    bytes
}

/// An MP4 of exactly `len` bytes: [`mp4`]'s boxes with the `mdat` padded.
fn mp4_of(len: usize) -> Vec<u8> {
    let head = mp4("");
    let mut bytes = head[..head.len() - unique("").len() - 8].to_vec();
    let rest = len - bytes.len();
    bytes.extend_from_slice(&(rest as u32).to_be_bytes());
    bytes.extend_from_slice(b"mdat");
    bytes.resize(len, 7);
    bytes
}

fn hash(bytes: &[u8]) -> String {
    dcl_auth::hash_bytes_v1(bytes)
}

/// A multipart form of `parts`, each `(field, filename, bytes)`.
fn form(parts: &[(&str, Option<&str>, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, filename, bytes) in parts {
        body.extend_from_slice(
            format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"").as_bytes(),
        );
        if let Some(filename) = filename {
            body.extend_from_slice(
                format!("; filename=\"{filename}\"\r\nContent-Type: video/mp4").as_bytes(),
            );
        }
        body.extend_from_slice(b"\r\n\r\n");
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

fn video_form(bytes: &[u8]) -> Vec<u8> {
    form(&[(VIDEO_FILE, Some("video.mp4"), bytes)])
}

/// A body that counts every byte the node pulls from it: `head`, then
/// `filler` bytes of 7 made as they are pulled, then `tail`; `stall` pends
/// forever after `head`.
fn counted(head: Vec<u8>, filler: u64, tail: Vec<u8>, stall: bool) -> (Body, Arc<AtomicU64>) {
    let pulled = Arc::new(AtomicU64::new(0));
    let chunk = 64 * 1024u64;
    let fill = futures::stream::iter(
        (0..filler.div_ceil(chunk))
            .map(move |i| Bytes::from(vec![7u8; chunk.min(filler - i * chunk) as usize])),
    );
    let stream = futures::stream::iter([Bytes::from(head)])
        .chain(fill)
        .chain(futures::stream::iter([Bytes::from(tail)]))
        .chain(futures::stream::pending().take(usize::from(stall)))
        .map({
            let pulled = pulled.clone();
            move |chunk: Bytes| {
                pulled.fetch_add(chunk.len() as u64, Ordering::SeqCst);
                Ok::<_, std::io::Error>(chunk)
            }
        });
    (Body::from_stream(stream), pulled)
}

/// A body that records being read and never yields a byte.
fn untouchable() -> (Body, Arc<AtomicU64>) {
    let polled = Arc::new(AtomicU64::new(0));
    let stream = futures::stream::poll_fn({
        let polled = polled.clone();
        move |_| {
            polled.fetch_add(1, Ordering::SeqCst);
            std::task::Poll::<Option<Result<Bytes, std::io::Error>>>::Pending
        }
    });
    (Body::from_stream(stream), polled)
}

/// Waits until `ready` holds, failing the test after 30 seconds.
async fn until(what: &str, ready: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(30), async {
        while !ready() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} never happened"));
}

struct Node {
    pool: PgPool,
    app: Router,
    state: crate::SharedState,
    storage: Arc<Storage>,
}

async fn node_with(configure: impl FnOnce(&mut crate::State), writes: bool) -> Option<Node> {
    node_on(super::support::scratch_pool(8).await?, configure, writes).await
}

async fn node_over(
    connections: u32,
    configure: impl FnOnce(&mut crate::State),
    writes: bool,
) -> Option<Node> {
    node_on(
        super::support::scratch_pool(connections).await?,
        configure,
        writes,
    )
    .await
}

/// A node on a database of its own, whose totals no other test moves.
async fn isolated_node(configure: impl FnOnce(&mut crate::State)) -> Option<Node> {
    let pool = crate::db::scratch::isolated_pool(8).await?;
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    node_on(pool, configure, true).await
}

async fn node_on(
    pool: PgPool,
    configure: impl FnOnce(&mut crate::State),
    writes: bool,
) -> Option<Node> {
    let committee = FakeCommittee::with(&[&address(COMMITTEE_KEY)]);
    let storage = temp_storage().await;
    let mut state = crate::State::new(
        pool.clone(),
        storage.clone(),
        Arc::default(),
        dcl_http::Outbound::for_tests(),
        Some(committee_index(&committee).await),
        None,
        crate::Settings {
            writes_enabled: writes,
            ..crate::Settings::default()
        },
    )
    .unwrap();
    configure(&mut state);
    let state = Arc::new(state);
    let app = Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state.clone()));
    Some(Node {
        pool,
        app,
        state,
        storage: storage.item_files().unwrap().clone(),
    })
}

async fn node() -> Option<Node> {
    node_with(|_| {}, true).await
}

/// `body` as a multipart upload of `path`, signed by `key` unless `None`,
/// from the source address `ip` if any.
async fn post(key: Option<&str>, path: &str, ip: Option<&str>, body: Body) -> Request<Body> {
    let mut request = Request::builder().method("POST").uri(path).header(
        header::CONTENT_TYPE,
        format!("multipart/form-data; boundary={BOUNDARY}"),
    );
    if let Some(key) = key {
        for (name, value) in signed_map(key, "post", path, "{}", false).await {
            request = request.header(name, value);
        }
    }
    if let Some(ip) = ip {
        request = request.header("x-real-ip", ip);
    }
    request.body(body).unwrap()
}

/// The status, JSON body and `Retry-After` of `response`.
async fn answer(response: axum::response::Response) -> (u16, Value, Option<String>) {
    let status = response.status().as_u16();
    let retry = response
        .headers()
        .get(header::RETRY_AFTER)
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        retry,
    )
}

/// [`post`] through the router, failing the test when the node takes a
/// minute to answer.
async fn send_from(
    app: &Router,
    key: Option<&str>,
    path: &str,
    ip: Option<&str>,
    body: Body,
) -> (u16, Value, Option<String>) {
    let request = post(key, path, ip, body).await;
    let response = tokio::time::timeout(Duration::from_secs(60), app.clone().oneshot(request))
        .await
        .expect("the node answers within a minute")
        .unwrap();
    answer(response).await
}

async fn send(app: &Router, key: Option<&str>, path: &str, body: Body) -> (u16, Value) {
    let (status, body, _) = send_from(app, key, path, None, body).await;
    (status, body)
}

/// [`post`] through the router with no bound of its own, for a test that
/// moves the clock.
async fn send_unbounded(app: &Router, key: &str, path: &str, body: Body) -> (u16, Value) {
    let response = app
        .clone()
        .oneshot(post(Some(key), path, None, body).await)
        .await
        .unwrap();
    let (status, body, _) = answer(response).await;
    (status, body)
}

async fn upload(node: &Node, key: Option<&str>, item: Uuid, body: Vec<u8>) -> (u16, Value) {
    send(
        &node.app,
        key,
        &format!("/v1/items/{item}/videos"),
        Body::from(body),
    )
    .await
}

/// Reads `hash` from the storage route, signed by `key` unless `None`, with
/// `range` if any: status, content type, cache control and body.
async fn read(
    node: &Node,
    key: Option<&str>,
    hash: &str,
    range: Option<&str>,
) -> (u16, String, String, Vec<u8>) {
    let path = format!("/v1/storage/contents/{hash}");
    let mut request = Request::builder().uri(&path);
    if let Some(key) = key {
        for (name, value) in signed_map(key, "get", &path, "{}", false).await {
            request = request.header(name, value);
        }
    }
    if let Some(range) = range {
        request = request.header(header::RANGE, range);
    }
    let response = node
        .app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let text = |name| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string()
    };
    let (status, kind, cache) = (
        response.status().as_u16(),
        text(header::CONTENT_TYPE),
        text(header::CACHE_CONTROL),
    );
    if response.status().is_success() {
        assert_eq!(
            (
                response.headers()[header::X_CONTENT_TYPE_OPTIONS]
                    .to_str()
                    .unwrap(),
                response.headers()[header::CONTENT_SECURITY_POLICY]
                    .to_str()
                    .unwrap(),
            ),
            ("nosniff", "sandbox"),
            "a served file is never sniffed or run as a page"
        );
    }
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 24)
        .await
        .unwrap();
    (status, kind, cache, bytes.to_vec())
}

/// A standard draft collection of the owner's with a smart wearable naming
/// `video` as its preview, as the `/create` dApp saves one.
async fn smart_wearable(node: &Node, video: &[u8]) -> (Uuid, Uuid) {
    let collection = Uuid::new_v4();
    save_collection(
        &node.state.items,
        &address(OWNER_KEY),
        json!({ "id": collection, "name": format!("Videos {}", &collection.simple().to_string()[..12]) }),
    )
    .await
    .unwrap();
    let item = Uuid::new_v4();
    name_video(node, collection, item, video).await;
    (collection, item)
}

/// [`smart_wearable`] owned by the wallet of `key`.
async fn smart_wearable_of(node: &Node, key: &str, video: &[u8]) -> Uuid {
    let (collection, item) = smart_wearable(node, video).await;
    sqlx::query("UPDATE builder.collections SET eth_address = $2 WHERE id = $1")
        .bind(collection)
        .bind(address(key))
        .execute(&node.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE builder.items SET eth_address = $2 WHERE id = $1")
        .bind(item)
        .bind(address(key))
        .execute(&node.pool)
        .await
        .unwrap();
    collection
}

/// Saves item `item` of `collection` naming `video` at `video.mp4`.
async fn name_video(node: &Node, collection: Uuid, item: Uuid, video: &[u8]) {
    let model = unique("model");
    saved_item(
        &node.state.items,
        &address(OWNER_KEY),
        &json!({
            "id": item, "name": "Smart hat", "collection_id": collection, "rarity": "common",
            "video": hash(video),
            "contents": { "model.glb": hash(&model), VIDEO_FILE: hash(video) },
        }),
    )
    .await;
}

async fn records(pool: &PgPool, hash: &str) -> Vec<(i64, String)> {
    sqlx::query_as("SELECT size, uploaded_by FROM builder.uploaded_contents WHERE hash = $1")
        .bind(hash)
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn stored(node: &Node, hash: &str) -> bool {
    node.storage.exist(hash).await.unwrap()
}

fn spool_entries(node: &Node) -> usize {
    std::fs::read_dir(video::spool_root(&node.storage)).map_or(0, |dir| dir.count())
}

async fn referenced(pool: &PgPool, hash: &str) -> bool {
    let mut conn = pool.acquire().await.unwrap();
    crate::storage::gc::referenced_rows_among_on(&mut conn, &[hash.to_string()])
        .await
        .unwrap()
        .contains(hash)
}

async fn cleanup(pool: &PgPool, collections: &[Uuid], hashes: &[String]) {
    sqlx::query("DELETE FROM builder.collections WHERE id = ANY($1)")
        .bind(collections)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM builder.uploaded_contents WHERE hash = ANY($1)")
        .bind(hashes)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_creator_uploads_the_video_their_smart_wearable_names_and_plays_it_back_signed() {
    let Some(node) = node().await else { return };
    let video = mp4("creator");
    let (collection, item) = smart_wearable(&node, &video).await;
    for mount in ["", "/builder-api"] {
        let body = form(&[(
            VIDEO_FILE,
            Some("../../etc/passwd%0D%0AX-Injected: 1<script>.html"),
            &video,
        )]);
        let (status, answer) = send(
            &node.app,
            Some(OWNER_KEY),
            &format!("{mount}/v1/items/{item}/videos"),
            Body::from(body),
        )
        .await;
        assert_eq!((status, answer), (200, json!({ "ok": true })), "{mount}");
    }
    assert_eq!(
        records(&node.pool, &hash(&video)).await,
        vec![(video.len() as i64, address(OWNER_KEY))]
    );
    assert!(stored(&node, &hash(&video)).await);
    assert_eq!(spool_entries(&node), 0, "the spool keeps nothing");

    let (status, kind, cache, bytes) = read(&node, Some(OWNER_KEY), &hash(&video), None).await;
    assert_eq!(
        (status, kind.as_str(), cache.as_str()),
        (200, "video/mp4", "private, no-store")
    );
    assert_eq!(bytes, video);
    let (status, _, _, bytes) =
        read(&node, Some(OWNER_KEY), &hash(&video), Some("bytes=4-7")).await;
    assert_eq!((status, bytes.as_slice()), (206, b"ftyp".as_slice()));
    assert_eq!(read(&node, None, &hash(&video), None).await.0, 404);
    assert_eq!(
        read(&node, Some(STRANGER_KEY), &hash(&video), None).await.0,
        404
    );
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

/// A video whose stored bytes the collector removes while the upload waits
/// for the store answers 503 and records nothing, as a file upload does;
/// sent again, it is stored and recorded.
#[tokio::test]
async fn a_video_the_collector_takes_while_its_upload_waits_answers_503_and_its_retry_keeps_it() {
    let Some(node) = isolated_node(|_| {}).await else {
        return;
    };
    let video = mp4("forgotten then sent again");
    let (collection, item) = smart_wearable(&node, &video).await;
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&video))
            .await
            .0,
        200
    );
    sqlx::query("DELETE FROM builder.uploaded_contents WHERE hash = $1")
        .bind(hash(&video))
        .execute(&node.pool)
        .await
        .unwrap();

    let batch = crate::storage::gc::lock_store_exclusive(&node.pool)
        .await
        .unwrap();
    let again = tokio::spawn({
        let app = node.app.clone();
        let body = video_form(&video);
        async move {
            send(
                &app,
                Some(OWNER_KEY),
                &format!("/v1/items/{item}/videos"),
                Body::from(body),
            )
            .await
        }
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !super::support::waiting_for_the_store(&node.pool).await {
        assert!(
            !again.is_finished(),
            "the upload committed while the collector's unlink batch held the store"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "the upload never asked for the store"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    node.storage.delete_strict(&hash(&video)).await.unwrap();
    batch.commit().await.unwrap();

    let (status, answer) = again.await.unwrap();
    assert_eq!(
        (status, answer["error"].as_str()),
        (
            503,
            Some("The content store is being collected; retry shortly")
        )
    );
    assert!(records(&node.pool, &hash(&video)).await.is_empty());
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&video))
            .await
            .0,
        200
    );
    assert_eq!(
        records(&node.pool, &hash(&video)).await,
        vec![(video.len() as i64, address(OWNER_KEY))]
    );
    assert!(stored(&node, &hash(&video)).await);
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

/// A video behind an unlink batch that outlasts the store lock's wait
/// answers 503 and records nothing; its bytes went to the store before the
/// upload asked for the lock, so no transaction writes them, and the
/// collector may take them.
#[tokio::test]
async fn a_video_behind_a_long_unlink_batch_answers_503_and_frees_its_place_and_spool() {
    let Some(node) = isolated_node(|_| {}).await else {
        return;
    };
    let video = mp4("behind a long unlink batch");
    let (collection, item) = smart_wearable(&node, &video).await;

    let batch = crate::storage::gc::lock_store_exclusive(&node.pool)
        .await
        .unwrap();
    let answered = tokio::time::timeout(
        Duration::from_secs(10),
        upload(&node, Some(OWNER_KEY), item, video_form(&video)),
    )
    .await
    .expect("bounded wait");
    batch.commit().await.unwrap();
    assert_eq!(
        (answered.0, answered.1["error"].as_str()),
        (
            503,
            Some("The content store is being collected; retry shortly")
        )
    );
    assert!(records(&node.pool, &hash(&video)).await.is_empty());
    assert!(
        stored(&node, &hash(&video)).await,
        "the bytes were not stored before the store's lock"
    );
    assert_eq!(spool_entries(&node), 0, "the spool keeps nothing");
    assert!(node.state.videos.idle(), "the place was kept");

    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&video))
            .await
            .0,
        200
    );
    assert!(stored(&node, &hash(&video)).await);
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

#[tokio::test]
async fn after_publication_and_approval_the_video_opens_unsigned_as_the_marketplace_reads_it() {
    let Some(node) = node().await else { return };
    let video = mp4("approved");
    let (collection, item) = smart_wearable(&node, &video).await;
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&video))
            .await
            .0,
        200
    );
    let contract = on_chain(&node.pool, collection, true).await;
    let (status, kind, cache, bytes) = read(&node, None, &hash(&video), None).await;
    assert_eq!(
        (status, kind.as_str(), cache.as_str()),
        (200, "video/mp4", "public,max-age=31536000,immutable")
    );
    assert_eq!(bytes, video);
    let (status, _, _, bytes) = read(&node, None, &hash(&video), Some("bytes=0-3")).await;
    assert_eq!((status, bytes.len()), (206, 4));
    super::support::off_chain(&node.pool, &contract).await;
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

#[tokio::test]
async fn nobody_without_a_right_to_the_item_makes_the_node_read_a_byte() {
    let Some(node) = node().await else { return };
    let video = mp4("refused");
    let (collection, item) = smart_wearable(&node, &video).await;
    let contract = on_chain(&node.pool, collection, false).await;
    sqlx::query("UPDATE shop.collections SET managers = $2 WHERE contract_address = $1")
        .bind(&contract)
        .bind(vec![address(OTHER_KEY)])
        .execute(&node.pool)
        .await
        .unwrap();
    let path = format!("/v1/items/{item}/videos");
    let validator = {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let signature = crate::shared::validator_signature::sign(
            b"example-validator-secret-of-32-bytes",
            &timestamp,
            "POST",
            &path,
            b"",
        );
        vec![
            (
                crate::shared::validator_signature::SIGNATURE_HEADER,
                signature,
            ),
            (
                crate::shared::validator_signature::TIMESTAMP_HEADER,
                timestamp,
            ),
        ]
    };
    for (who, key, extra) in [
        ("another creator", Some(STRANGER_KEY), vec![]),
        ("a former manager", Some(MANAGER_KEY), vec![]),
        ("a committee member", Some(COMMITTEE_KEY), vec![]),
        ("an unsigned caller", None, vec![]),
        ("a validator-signed caller", None, validator),
    ] {
        let (body, polled) = untouchable();
        let mut request = Request::builder().method("POST").uri(&path).header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        );
        if let Some(key) = key {
            for (name, value) in signed_map(key, "post", &path, "{}", false).await {
                request = request.header(name, value);
            }
        }
        for (name, value) in extra {
            request = request.header(name, value);
        }
        let response = node
            .app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 401, "{who}");
        assert_eq!(polled.load(Ordering::SeqCst), 0, "{who}: the body was read");
    }
    let (body, polled) = untouchable();
    assert_eq!(
        send(
            &node.app,
            Some(OWNER_KEY),
            &format!("/v1/items/{}/videos", Uuid::new_v4()),
            body
        )
        .await
        .0,
        404
    );
    assert_eq!(
        polled.load(Ordering::SeqCst),
        0,
        "an unknown item's body was read"
    );
    assert!(records(&node.pool, &hash(&video)).await.is_empty());
    assert!(!stored(&node, &hash(&video)).await);
    super::support::off_chain(&node.pool, &contract).await;
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

#[tokio::test]
async fn a_current_on_chain_manager_uploads_like_the_owner() {
    let Some(node) = node().await else { return };
    let video = mp4("manager");
    let (collection, item) = smart_wearable(&node, &video).await;
    let contract = on_chain(&node.pool, collection, false).await;
    sqlx::query("UPDATE shop.collections SET managers = $2 WHERE contract_address = $1")
        .bind(&contract)
        .bind(vec![address(MANAGER_KEY)])
        .execute(&node.pool)
        .await
        .unwrap();
    assert_eq!(
        upload(&node, Some(MANAGER_KEY), item, video_form(&video))
            .await
            .0,
        200
    );
    assert_eq!(
        records(&node.pool, &hash(&video)).await,
        vec![(video.len() as i64, address(MANAGER_KEY))]
    );
    super::support::off_chain(&node.pool, &contract).await;
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

#[tokio::test]
async fn a_video_at_the_cap_is_kept_and_one_byte_over_stops_there_leaving_nothing() {
    let Some(node) = node_with(|state| state.videos.max_bytes = 4096, true).await else {
        return;
    };
    let at_cap = mp4_of(4096);
    let over = mp4_of(4097);
    let (collection, item) = smart_wearable(&node, &over).await;
    let (status, answer) = upload(&node, Some(OWNER_KEY), item, video_form(&over)).await;
    assert_eq!(
        (status, answer),
        (
            413,
            json!({ "ok": false, "data": { "maxBytes": 4096 }, "error": "The video is too large" })
        )
    );
    assert!(!stored(&node, &hash(&over)).await);
    assert!(records(&node.pool, &hash(&over)).await.is_empty());
    assert_eq!(spool_entries(&node), 0);

    name_video(&node, collection, item, &at_cap).await;
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&at_cap))
            .await
            .0,
        200
    );
    assert_eq!(records(&node.pool, &hash(&at_cap)).await.len(), 1);
    cleanup(&node.pool, &[collection], &[hash(&at_cap), hash(&over)]).await;
}

#[tokio::test]
async fn the_default_cap_is_upstreams_250_mb_and_the_node_stops_reading_past_it() {
    let Some(node) = node().await else { return };
    assert_eq!(node.state.videos.max_bytes, 250_000_000);
    let video = mp4("default cap");
    let (collection, item) = smart_wearable(&node, &video).await;
    let head = form(&[(VIDEO_FILE, Some("video.mp4"), &mp4_of(64))]);
    let head = head[..head.len() - format!("\r\n--{BOUNDARY}--\r\n").len()].to_vec();
    let (body, pulled) = counted(
        head,
        250_000_000,
        b"\r\n--videoboundary--\r\n".to_vec(),
        false,
    );
    let (status, answer) = send(
        &node.app,
        Some(OWNER_KEY),
        &format!("/v1/items/{item}/videos"),
        body,
    )
    .await;
    assert_eq!(
        (status, &answer["data"]),
        (413, &json!({ "maxBytes": 250_000_000 }))
    );
    assert!(
        (250_000_000..250_000_064 + 256 * 1024).contains(&pulled.load(Ordering::SeqCst)),
        "read {} bytes",
        pulled.load(Ordering::SeqCst)
    );
    assert_eq!(spool_entries(&node), 0);
    cleanup(&node.pool, &[collection], &[]).await;
}

#[tokio::test]
async fn a_file_that_is_not_a_playable_mp4_is_refused_and_not_stored() {
    let Some(node) = node().await else { return };
    let page = b"<!DOCTYPE html><script>alert(document.cookie)</script>".to_vec();
    let mut truncated = mp4("truncated");
    truncated.truncate(truncated.len() - 3);
    let mut moovless = Vec::new();
    moovless.extend_from_slice(&mp4("moovless")[..24]);
    moovless.extend_from_slice(b"\0\0\0\x0cmdatdata");
    let (collection, item) = smart_wearable(&node, &page).await;
    for (what, bytes) in [
        ("a page", &page),
        ("a truncated container", &truncated),
        ("no moov", &moovless),
    ] {
        name_video(&node, collection, item, bytes).await;
        let (status, answer) = upload(&node, Some(OWNER_KEY), item, video_form(bytes)).await;
        assert_eq!(
            (status, answer),
            (
                400,
                json!({ "ok": false, "data": {}, "error": "The video is not an MP4 file" })
            ),
            "{what}"
        );
        assert!(!stored(&node, &hash(bytes)).await, "{what}");
        assert!(records(&node.pool, &hash(bytes)).await.is_empty(), "{what}");
    }
    name_video(&node, collection, item, &page).await;
    let (status, answer) = send(
        &node.app,
        Some(OWNER_KEY),
        &format!("/v1/items/{item}/files"),
        Body::from(form(&[(&hash(&page), Some("blob"), &page)])),
    )
    .await;
    assert_eq!(
        (status, answer),
        (
            400,
            json!({ "ok": false, "data": { "hash": hash(&page) }, "error": "The video is uploaded through the videos route" })
        ),
        "the item's video sent as a file"
    );
    assert!(!stored(&node, &hash(&page)).await);
    assert!(records(&node.pool, &hash(&page)).await.is_empty());
    let head = form(&[(VIDEO_FILE, Some("video.mp4"), &page)]);
    let head = head[..head.len() - format!("\r\n--{BOUNDARY}--\r\n").len()].to_vec();
    let (body, pulled) = counted(
        head,
        8 * 1024 * 1024,
        b"\r\n--videoboundary--\r\n".to_vec(),
        false,
    );
    let (status, _) = send(
        &node.app,
        Some(OWNER_KEY),
        &format!("/v1/items/{item}/videos"),
        body,
    )
    .await;
    assert_eq!(status, 400, "a page with a long tail");
    assert!(
        pulled.load(Ordering::SeqCst) < 1024 * 1024,
        "the node read {} bytes of a page",
        pulled.load(Ordering::SeqCst)
    );
    assert_eq!(spool_entries(&node), 0);
    cleanup(&node.pool, &[collection], &[]).await;
}

#[tokio::test]
async fn a_second_video_replaces_the_first_which_the_collector_may_then_take() {
    let Some(node) = node().await else { return };
    let (first, second) = (mp4("first"), mp4("second"));
    let (collection, item) = smart_wearable(&node, &first).await;
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&first))
            .await
            .0,
        200
    );
    assert!(referenced(&node.pool, &hash(&first)).await);
    name_video(&node, collection, item, &second).await;
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&second))
            .await
            .0,
        200
    );
    let named: (String, Option<String>) = sqlx::query_as(
        "SELECT c.hash, i.video FROM builder.item_contents c JOIN builder.items i ON i.id = c.item_id \
         WHERE c.item_id = $1 AND c.file = $2",
    )
    .bind(item)
    .bind(VIDEO_FILE)
    .fetch_one(&node.pool)
    .await
    .unwrap();
    assert_eq!(named, (hash(&second), Some(hash(&second))));
    assert!(referenced(&node.pool, &hash(&second)).await);
    assert!(
        !referenced(&node.pool, &hash(&first)).await,
        "the replaced video stays referenced"
    );
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&first))
            .await
            .0,
        400,
        "the item no longer names it"
    );
    cleanup(&node.pool, &[collection], &[hash(&first), hash(&second)]).await;
}

#[tokio::test]
async fn uploads_follow_the_item_and_the_switch_not_the_lock_or_publication() {
    let Some(node) = node().await else { return };
    let video = mp4("states");
    let (collection, item) = smart_wearable(&node, &video).await;
    sqlx::query("UPDATE builder.collections SET lock = timezone('UTC', now()) WHERE id = $1")
        .bind(collection)
        .execute(&node.pool)
        .await
        .unwrap();
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&video))
            .await
            .0,
        200,
        "locked"
    );
    let contract = on_chain(&node.pool, collection, true).await;
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&video))
            .await
            .0,
        200,
        "published"
    );
    super::support::off_chain(&node.pool, &contract).await;
    sqlx::query("DELETE FROM builder.items WHERE id = $1")
        .bind(item)
        .execute(&node.pool)
        .await
        .unwrap();
    let (body, polled) = untouchable();
    assert_eq!(
        send(
            &node.app,
            Some(OWNER_KEY),
            &format!("/v1/items/{item}/videos"),
            body
        )
        .await
        .0,
        404,
        "deleted"
    );
    assert_eq!(polled.load(Ordering::SeqCst), 0);

    let Some(off) = node_with(|_| {}, false).await else {
        return;
    };
    let (body, polled) = untouchable();
    let (status, answer) = send(
        &off.app,
        Some(OWNER_KEY),
        &format!("/v1/items/{item}/videos"),
        body,
    )
    .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (503, Some(crate::writes::DISABLED_MESSAGE))
    );
    assert_eq!(polled.load(Ordering::SeqCst), 0);
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

#[tokio::test]
async fn malformed_forms_are_refused_and_store_nothing() {
    let Some(node) = node().await else { return };
    let video = mp4("forms");
    let (collection, item) = smart_wearable(&node, &video).await;
    let long_name = "n".repeat(2 * 1024 * 1024);
    let cases: Vec<(&str, Vec<u8>, u16)> = vec![
        (
            "two file parts",
            form(&[
                (VIDEO_FILE, Some("a.mp4"), &video),
                (VIDEO_FILE, Some("b.mp4"), &video),
            ]),
            400,
        ),
        (
            "a part after the file",
            form(&[(VIDEO_FILE, Some("a.mp4"), &video), ("note", None, b"x")]),
            400,
        ),
        (
            "an unknown field",
            form(&[("model.glb", Some("a.mp4"), &video)]),
            400,
        ),
        (
            "the hash as the field",
            form(&[(&hash(&video), Some("a.mp4"), &video)]),
            400,
        ),
        ("no file part", form(&[]), 400),
        (
            "a field, not a file",
            form(&[(VIDEO_FILE, None, &video)]),
            400,
        ),
        (
            "a 2 MiB field name",
            form(&[(&long_name, Some("a.mp4"), &video)]),
            400,
        ),
        ("not multipart", video.clone(), 400),
    ];
    for (what, body, expected) in cases {
        let (status, _) = upload(&node, Some(OWNER_KEY), item, body).await;
        assert_eq!(status, expected, "{what}");
        assert!(!stored(&node, &hash(&video)).await, "{what}");
        assert!(
            records(&node.pool, &hash(&video)).await.is_empty(),
            "{what}"
        );
    }
    let mut preamble = vec![b'x'; 2 * 1024 * 1024];
    preamble.extend_from_slice(b"\r\n");
    let (body, pulled) = counted(preamble, 0, video_form(&video), false);
    let (status, _) = send(
        &node.app,
        Some(OWNER_KEY),
        &format!("/v1/items/{item}/videos"),
        body,
    )
    .await;
    assert_eq!(status, 400, "a 2 MiB preamble");
    assert!(
        pulled.load(Ordering::SeqCst) <= 2 * 1024 * 1024 + 2,
        "the preamble was read whole"
    );
    assert_eq!(spool_entries(&node), 0);
    cleanup(&node.pool, &[collection], &[]).await;
}

/// A body the test feeds: its sender and the bytes the node has pulled.
fn fed() -> (
    Body,
    tokio::sync::mpsc::UnboundedSender<Bytes>,
    Arc<AtomicU64>,
) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Bytes>();
    let pulled = Arc::new(AtomicU64::new(0));
    let counter = pulled.clone();
    let body = Body::from_stream(futures::stream::unfold(rx, move |mut rx| {
        let counter = counter.clone();
        async move {
            let chunk = rx.recv().await?;
            counter.fetch_add(chunk.len() as u64, Ordering::SeqCst);
            Some((Ok::<_, std::io::Error>(chunk), rx))
        }
    }));
    (body, tx, pulled)
}

/// Lets every ready task run, without a timer.
async fn settle() {
    for _ in 0..1_000 {
        tokio::task::yield_now().await;
    }
}

/// Spins until `pulled` reaches `bytes` or `ended` holds, without a timer,
/// so it holds under a paused clock.
async fn pulled_to(pulled: &AtomicU64, bytes: usize, ended: impl Fn() -> bool) {
    for _ in 0..1_000_000 {
        if pulled.load(Ordering::SeqCst) >= bytes as u64 || ended() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("the node never read {bytes} bytes");
}

/// An upload of `item`'s `video` whose body the test feeds in steps of
/// `step` paused seconds, `rounds` times, `per_step` bytes each; then the
/// rest.
async fn fed_upload(
    node: &Node,
    item: Uuid,
    video: &[u8],
    step: u64,
    rounds: usize,
    per_step: usize,
) -> (u16, Value) {
    let (body, tx, pulled) = fed();
    let uploading = tokio::spawn({
        let app = node.app.clone();
        async move { send_unbounded(&app, OWNER_KEY, &format!("/v1/items/{item}/videos"), body).await }
    });
    let whole = video_form(video);
    let mut sent = 120;
    tx.send(Bytes::copy_from_slice(&whole[..sent])).unwrap();
    until("the upload's spool file", || spool_entries(node) == 1).await;
    pulled_to(&pulled, sent, || uploading.is_finished()).await;
    tokio::time::pause();
    for _ in 0..rounds {
        tokio::time::advance(Duration::from_secs(step)).await;
        if tx
            .send(Bytes::copy_from_slice(&whole[sent..sent + per_step]))
            .is_err()
        {
            break;
        }
        sent += per_step;
        pulled_to(&pulled, sent, || uploading.is_finished()).await;
    }
    tokio::time::resume();
    let _ = tx.send(Bytes::copy_from_slice(&whole[sent..]));
    drop(tx);
    tokio::time::timeout(Duration::from_secs(10), uploading)
        .await
        .expect("the upload ended")
        .unwrap()
}

#[tokio::test]
async fn a_sender_that_stalls_is_cut_at_the_idle_bound_and_leaves_nothing() {
    let Some(node) = node().await else { return };
    assert_eq!(
        (node.state.videos.deadline, node.state.videos.idle_timeout),
        (Duration::from_secs(1200), Duration::from_secs(30))
    );
    let video = mp4("stalled");
    let (collection, item) = smart_wearable(&node, &video).await;
    let (body, tx, pulled) = fed();
    let uploading = tokio::spawn({
        let app = node.app.clone();
        async move { send_unbounded(&app, OWNER_KEY, &format!("/v1/items/{item}/videos"), body).await }
    });
    tx.send(Bytes::copy_from_slice(&video_form(&video)[..120]))
        .unwrap();
    until("the upload's spool file", || spool_entries(&node) == 1).await;
    pulled_to(&pulled, 120, || false).await;
    tokio::time::pause();
    settle().await;
    tokio::time::advance(Duration::from_secs(29)).await;
    settle().await;
    assert!(!uploading.is_finished(), "cut before the idle bound");
    tokio::time::advance(Duration::from_secs(2)).await;
    settle().await;
    tokio::time::resume();
    let (status, answer) = tokio::time::timeout(Duration::from_secs(10), uploading)
        .await
        .expect("a stalled upload is cut at the idle bound")
        .unwrap();
    assert_eq!(
        (status, answer["error"].as_str()),
        (408, Some("The upload timed out"))
    );
    assert_eq!(spool_entries(&node), 0);
    assert!(records(&node.pool, &hash(&video)).await.is_empty());
    assert!(node.state.videos.idle(), "the place was kept");
    drop(tx);
    cleanup(&node.pool, &[collection], &[]).await;
}

#[tokio::test]
async fn a_small_video_on_a_slow_link_finishes_while_the_node_has_places() {
    let Some(node) = node().await else { return };
    let video = mp4("slow and steady");
    let (collection, item) = smart_wearable(&node, &video).await;
    assert_eq!(
        fed_upload(&node, item, &video, 20, 6, 1).await,
        (200, json!({ "ok": true }))
    );
    assert_eq!(records(&node.pool, &hash(&video)).await.len(), 1);
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

/// The node checks an upload's budgets after it takes its place and before
/// it reads the body. While those checks wait on the database, the idle
/// bound doesn't run: once they answer, the upload still has the whole
/// bound for its first bytes, sent a second after, and its video is kept.
#[tokio::test]
async fn an_upload_whose_budget_checks_wait_still_has_its_idle_bound_for_its_first_bytes() {
    let Some(node) =
        isolated_node(|state| state.videos.idle_timeout = Duration::from_secs(2)).await
    else {
        return;
    };
    let video = mp4("checked slowly");
    let (collection, item) = smart_wearable(&node, &video).await;
    let mut lock = node.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE builder.uploaded_contents IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let (body, tx, _) = fed();
    let uploading = tokio::spawn({
        let app = node.app.clone();
        async move {
            send(
                &app,
                Some(OWNER_KEY),
                &format!("/v1/items/{item}/videos"),
                body,
            )
            .await
        }
    });
    until("the upload's place", || node.state.videos.in_flight() == 1).await;
    let waiting = || async {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE wait_event_type = 'Lock' \
             AND query LIKE '%builder.uploaded_contents%' AND datname = current_database())",
        )
        .fetch_one(&node.pool)
        .await
        .unwrap()
    };
    while !waiting().await {
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_secs(3)).await;
    lock.commit().await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let _ = tx.send(Bytes::from(video_form(&video)));
    drop(tx);
    let answered = tokio::time::timeout(Duration::from_secs(10), uploading)
        .await
        .expect("the upload ended")
        .unwrap();
    assert_eq!(answered, (200, json!({ "ok": true })));
    assert_eq!(records(&node.pool, &hash(&video)).await.len(), 1);
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

#[tokio::test]
async fn a_steady_sender_past_the_total_is_cut_and_frees_its_place() {
    let Some(node) = node_with(
        |state| {
            state.videos.deadline = Duration::from_secs(90);
        },
        true,
    )
    .await
    else {
        return;
    };
    let video = mp4("too slow");
    let (collection, item) = smart_wearable(&node, &video).await;
    let (status, answer) = fed_upload(&node, item, &video, 20, 6, 1).await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (408, Some("The upload timed out"))
    );
    assert_eq!(spool_entries(&node), 0);
    assert!(records(&node.pool, &hash(&video)).await.is_empty());
    assert!(node.state.videos.idle(), "the place was kept");
    cleanup(&node.pool, &[collection], &[]).await;
}

#[tokio::test]
async fn concurrent_uploads_are_bounded_and_the_item_ends_naming_exactly_one() {
    let Some(node) = node().await else { return };
    let (first, second) = (mp4("racing first"), mp4("racing second"));
    let (collection, item) = smart_wearable(&node, &first).await;
    let path = format!("/v1/items/{item}/videos");
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Bytes>();
    let paused = Body::from_stream(futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|b| (Ok::<_, std::io::Error>(b), rx))
    }));
    let racing = tokio::spawn({
        let app = node.app.clone();
        let path = path.clone();
        async move { send(&app, Some(OWNER_KEY), &path, paused).await }
    });
    let whole = video_form(&first);
    let split = whole.len() / 2;
    tx.send(Bytes::copy_from_slice(&whole[..split])).unwrap();
    until("the first upload's admission", || !node.state.videos.idle()).await;

    let (body, polled) = untouchable();
    let (status, answer) = send(&node.app, Some(OWNER_KEY), &path, body).await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (
            429,
            Some("Too many video uploads of this wallet in progress")
        )
    );
    assert_eq!(polled.load(Ordering::SeqCst), 0);
    name_video(&node, collection, item, &second).await;
    tx.send(Bytes::copy_from_slice(&whole[split..])).unwrap();
    drop(tx);
    let (status, _) = racing.await.unwrap();
    assert_eq!(status, 400, "the first video is no longer named");
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&second))
            .await
            .0,
        200
    );
    assert!(!stored(&node, &hash(&first)).await);
    assert!(records(&node.pool, &hash(&first)).await.is_empty());
    assert_eq!(records(&node.pool, &hash(&second)).await.len(), 1);
    assert_eq!(spool_entries(&node), 0);
    assert!(node.state.videos.idle());
    cleanup(&node.pool, &[collection], &[hash(&second)]).await;
}

/// The one item of `collection`.
async fn item_of(node: &Node, collection: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM builder.items WHERE collection_id = $1")
        .bind(collection)
        .fetch_one(&node.pool)
        .await
        .unwrap()
}

/// Holds a place for `key`'s upload of a new smart wearable from `ip`, its
/// body stalled after the part headers and 40 bytes of the video: the task
/// and the item's collection.
async fn hold(node: &Node, key: &str, ip: &str) -> (tokio::task::JoinHandle<()>, Uuid) {
    let video = mp4("held");
    let collection = smart_wearable_of(node, key, &video).await;
    let item = item_of(node, collection).await;
    let before = node.state.videos.in_flight();
    let whole = video_form(&video);
    let (body, _) = counted(whole[..headers_end(&whole) + 40].to_vec(), 0, vec![], true);
    let app = node.app.clone();
    let (key, ip) = (key.to_string(), ip.to_string());
    let task = tokio::spawn(async move {
        let request = post(
            Some(&key),
            &format!("/v1/items/{item}/videos"),
            Some(&ip),
            body,
        )
        .await;
        let _ = app.oneshot(request).await;
    });
    until("a held place", || {
        node.state.videos.in_flight() == before + 1
    })
    .await;
    (task, collection)
}

async fn release(node: &Node, held: Vec<(tokio::task::JoinHandle<()>, Uuid)>) -> Vec<Uuid> {
    let mut collections = Vec::new();
    for (task, collection) in held {
        task.abort();
        let _ = task.await;
        collections.push(collection);
    }
    until("every release", || node.state.videos.idle()).await;
    collections
}

/// `key`'s upload of a new smart wearable's video from `ip`: status, error,
/// `Retry-After` and the item's collection.
async fn another_upload(
    node: &Node,
    key: &str,
    ip: &str,
) -> (u16, Option<String>, Option<String>, Uuid) {
    another_upload_of(node, key, ip, &mp4("another")).await
}

/// Bytes past what the largest video's pace brings in one idle bound
/// (250 MB over 20 minutes, for 30 seconds: 6,250,000).
const EARNS_A_CUT: usize = 6_500_000;

/// [`another_upload`] of `video`.
async fn another_upload_of(
    node: &Node,
    key: &str,
    ip: &str,
    video: &[u8],
) -> (u16, Option<String>, Option<String>, Uuid) {
    let video = video.to_vec();
    let collection = smart_wearable_of(node, key, &video).await;
    let item = item_of(node, collection).await;
    let (status, answer, retry) = send_from(
        &node.app,
        Some(key),
        &format!("/v1/items/{item}/videos"),
        Some(ip),
        Body::from(video_form(&video)),
    )
    .await;
    (
        status,
        answer["error"].as_str().map(str::to_string),
        retry,
        collection,
    )
}

const WALLET_KEYS: [&str; 5] = [
    "0x8501010101010101010101010101010101010101010101010101010101010101",
    "0x8502020202020202020202020202020202020202020202020202020202020202",
    "0x8503030303030303030303030303030303030303030303030303030303030303",
    "0x8504040404040404040404040404040404040404040404040404040404040404",
    "0x8505050505050505050505050505050505050505050505050505050505050505",
];

#[tokio::test]
async fn wallets_and_an_address_at_their_places_leave_another_creator_room() {
    let Some(node) = node().await else { return };
    let mut held = Vec::new();
    for key in [WALLET_KEYS[0], WALLET_KEYS[1]] {
        held.push(hold(&node, key, "198.51.100.1").await);
    }
    let mut others = Vec::new();
    for (key, ip, error) in [
        (
            WALLET_KEYS[0],
            "198.51.100.9",
            "Too many video uploads of this wallet in progress",
        ),
        (
            WALLET_KEYS[2],
            "198.51.100.1",
            "Too many video uploads from this address in progress",
        ),
    ] {
        let (status, refused, retry, collection) = another_upload(&node, key, ip).await;
        assert_eq!(
            (status, refused.as_deref(), retry.as_deref()),
            (429, Some(error), Some("30")),
            "{key} from {ip}"
        );
        others.push(collection);
    }
    let (status, _, _, collection) = another_upload(&node, WALLET_KEYS[2], "198.51.100.3").await;
    assert_eq!(
        status, 200,
        "a third creator uploads beside the two wallets"
    );
    others.push(collection);
    others.extend(release(&node, held).await);
    cleanup(&node.pool, &others, &[]).await;
}

#[tokio::test]
async fn behind_a_gateway_the_node_does_not_trust_every_creator_shares_no_source_place() {
    let Some(node) = node().await else { return };
    let mut held = Vec::new();
    for key in &WALLET_KEYS[..3] {
        held.push(hold(&node, key, "10.0.0.7").await);
    }
    assert_eq!(node.state.videos.in_flight(), 3);
    let collections = release(&node, held).await;
    cleanup(&node.pool, &collections, &[]).await;
}

async fn hold_files(node: &Node, key: &str, ip: &str) -> (tokio::task::JoinHandle<()>, Uuid) {
    let collection = smart_wearable_of(node, key, &mp4("held files")).await;
    let item = item_of(node, collection).await;
    let before = node.state.file_uploads.in_flight();
    let (body, _) = counted(b"--videoboundary\r\n".to_vec(), 0, vec![], true);
    let app = node.app.clone();
    let (key, ip) = (key.to_string(), ip.to_string());
    let task = tokio::spawn(async move {
        let request = post(
            Some(&key),
            &format!("/v1/items/{item}/files"),
            Some(&ip),
            body,
        )
        .await;
        let _ = app.oneshot(request).await;
    });
    until("a held file upload", || {
        node.state.file_uploads.in_flight() == before + 1
    })
    .await;
    (task, collection)
}

#[tokio::test]
async fn file_uploads_in_flight_are_bounded_per_wallet_and_refused_before_their_body() {
    let Some(node) = node().await else { return };
    let mut held = Vec::new();
    for n in 0..crate::FILE_UPLOADS_PER_SIGNER {
        held.push(hold_files(&node, WALLET_KEYS[0], &format!("198.51.100.{}", n + 1)).await);
    }
    let collection = smart_wearable_of(&node, WALLET_KEYS[0], &mp4("one more")).await;
    let item = item_of(&node, collection).await;
    let (body, polled) = untouchable();
    let (status, answer, retry) = send_from(
        &node.app,
        Some(WALLET_KEYS[0]),
        &format!("/v1/items/{item}/files"),
        Some("198.51.100.9"),
        body,
    )
    .await;
    assert_eq!(
        (status, answer["error"].as_str(), retry.as_deref()),
        (
            429,
            Some("Too many file uploads of this wallet in progress"),
            Some("30")
        )
    );
    assert_eq!(
        polled.load(Ordering::SeqCst),
        0,
        "the refused body was read"
    );
    held.push(hold_files(&node, WALLET_KEYS[1], "198.51.100.9").await);
    let mut collections = vec![collection];
    for (task, held_collection) in held {
        task.abort();
        let _ = task.await;
        collections.push(held_collection);
    }
    until("every file upload's release", || {
        node.state.file_uploads.in_flight() == 0
    })
    .await;
    cleanup(&node.pool, &collections, &[]).await;
}

#[tokio::test]
async fn the_node_reads_at_most_its_places_of_videos_and_refuses_the_next_at_once() {
    let Some(node) = node_with(
        |state| {
            state.videos.places = crate::items::places::Places::new("video uploads", 4, 1, 2);
            state.videos.deadline = Duration::from_secs(1_000_000_000);
        },
        true,
    )
    .await
    else {
        return;
    };
    let mut held = Vec::new();
    for (n, key) in WALLET_KEYS[..4].iter().enumerate() {
        held.push(hold(&node, key, &format!("198.51.100.{}", n + 1)).await);
    }
    assert_eq!(node.state.videos.in_flight(), 4);
    let video = mp4("ninth");
    let collection = smart_wearable_of(&node, WALLET_KEYS[4], &video).await;
    let item = item_of(&node, collection).await;
    let (body, polled) = untouchable();
    let (status, answer, retry) = send_from(
        &node.app,
        Some(WALLET_KEYS[4]),
        &format!("/v1/items/{item}/videos"),
        Some("198.51.100.5"),
        body,
    )
    .await;
    assert_eq!(
        (status, answer["error"].as_str(), retry.as_deref()),
        (
            429,
            Some("Too many video uploads in progress, try again later"),
            Some("30")
        )
    );
    assert_eq!(polled.load(Ordering::SeqCst), 0);
    let mut collections = release(&node, held).await;
    assert_eq!(
        send_from(
            &node.app,
            Some(WALLET_KEYS[4]),
            &format!("/v1/items/{item}/videos"),
            Some("198.51.100.5"),
            Body::from(video_form(&video))
        )
        .await
        .0,
        200
    );
    collections.push(collection);
    cleanup(&node.pool, &collections, &[hash(&video)]).await;
}

#[tokio::test]
async fn a_spool_left_by_a_crash_is_removed_by_the_first_upload_after_it_ages() {
    let Some(node) = node().await else { return };
    let root = video::spool_root(&node.storage);
    std::fs::create_dir_all(&root).unwrap();
    let (old, fresh) = (root.join("old-crash"), root.join("live-upload"));
    std::fs::write(&old, b"left over").unwrap();
    std::fs::write(&fresh, b"in flight").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(
            std::time::SystemTime::now() - video::SPOOL_ORPHAN_AGE - Duration::from_secs(1),
        )
        .unwrap();
    let video = mp4("sweep");
    let (collection, item) = smart_wearable(&node, &video).await;
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&video))
            .await
            .0,
        200
    );
    assert!(!old.exists(), "the orphan stays");
    assert!(fresh.exists(), "a live upload's spool was removed");
    assert!(node.state.videos.deadline < video::SPOOL_ORPHAN_AGE);

    std::fs::File::options()
        .write(true)
        .open(&fresh)
        .unwrap()
        .set_modified(
            std::time::SystemTime::now() - video::SPOOL_ORPHAN_AGE - Duration::from_secs(1),
        )
        .unwrap();
    let later = mp4("a later upload");
    name_video(&node, collection, item, &later).await;
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&later))
            .await
            .0,
        200
    );
    assert!(
        !fresh.exists(),
        "a leftover too young at the first sweep stays once it has aged"
    );
    cleanup(&node.pool, &[collection], &[hash(&video), hash(&later)]).await;
}

#[tokio::test]
async fn a_video_larger_than_the_file_uploads_bound_is_kept_with_its_size() {
    let Some(node) = node().await else { return };
    let video = mp4_of(21 * 1024 * 1024);
    let (collection, item) = smart_wearable(&node, &video).await;
    assert_eq!(
        upload(&node, Some(OWNER_KEY), item, video_form(&video)).await,
        (200, json!({ "ok": true }))
    );
    assert_eq!(
        records(&node.pool, &hash(&video)).await,
        vec![(video.len() as i64, address(OWNER_KEY))]
    );
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

/// A body whose bytes the test sends when it chooses.
fn paused() -> (Body, tokio::sync::mpsc::UnboundedSender<Bytes>) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Bytes>();
    let body = Body::from_stream(futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|b| (Ok::<_, std::io::Error>(b), rx))
    }));
    (body, tx)
}

#[tokio::test]
async fn an_item_that_names_no_video_is_refused_before_its_body() {
    let Some(node) = node().await else { return };
    let video = mp4("unnamed");
    let (collection, item) = smart_wearable(&node, &video).await;
    sqlx::query("DELETE FROM builder.item_contents WHERE item_id = $1 AND file = $2")
        .bind(item)
        .bind(VIDEO_FILE)
        .execute(&node.pool)
        .await
        .unwrap();
    let (body, polled) = untouchable();
    let (status, answer) = send(
        &node.app,
        Some(OWNER_KEY),
        &format!("/v1/items/{item}/videos"),
        body,
    )
    .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (400, Some("The item names no video"))
    );
    assert_eq!(polled.load(Ordering::SeqCst), 0);
    cleanup(&node.pool, &[collection], &[]).await;
}

#[tokio::test]
async fn a_manager_removed_while_the_video_streams_stores_nothing() {
    let Some(node) = node().await else { return };
    let video = mp4("revoked");
    let (collection, item) = smart_wearable(&node, &video).await;
    let contract = on_chain(&node.pool, collection, false).await;
    let managers = |list: Vec<String>| {
        sqlx::query("UPDATE shop.collections SET managers = $2 WHERE contract_address = $1")
            .bind(contract.clone())
            .bind(list)
            .execute(&node.pool)
    };
    managers(vec![address(MANAGER_KEY)]).await.unwrap();
    let (body, tx) = paused();
    let uploading = tokio::spawn({
        let app = node.app.clone();
        async move {
            send(
                &app,
                Some(MANAGER_KEY),
                &format!("/v1/items/{item}/videos"),
                body,
            )
            .await
        }
    });
    let whole = video_form(&video);
    tx.send(Bytes::copy_from_slice(&whole[..10])).unwrap();
    until("the manager's admission", || !node.state.videos.idle()).await;
    managers(vec![]).await.unwrap();
    tx.send(Bytes::copy_from_slice(&whole[10..])).unwrap();
    drop(tx);
    assert_eq!(uploading.await.unwrap().0, 401);
    assert!(records(&node.pool, &hash(&video)).await.is_empty());
    assert!(!stored(&node, &hash(&video)).await);
    super::support::off_chain(&node.pool, &contract).await;
    cleanup(&node.pool, &[collection], &[]).await;
}

#[tokio::test]
async fn no_connection_or_row_lock_is_held_while_the_video_streams() {
    let Some(node) = node_over(1, |_| {}, true).await else {
        return;
    };
    let video = mp4("streaming");
    let (collection, item) = smart_wearable(&node, &video).await;
    let (body, tx) = paused();
    let uploading = tokio::spawn({
        let app = node.app.clone();
        async move {
            send(
                &app,
                Some(OWNER_KEY),
                &format!("/v1/items/{item}/videos"),
                body,
            )
            .await
        }
    });
    let whole = video_form(&video);
    tx.send(Bytes::copy_from_slice(&whole[..10])).unwrap();
    until("the admission", || !node.state.videos.idle()).await;
    let mut tx_db = tokio::time::timeout(Duration::from_secs(10), node.pool.begin())
        .await
        .expect("the upload holds the only connection")
        .unwrap();
    let locked: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM builder.items WHERE id = $1 FOR UPDATE NOWAIT")
            .bind(item)
            .fetch_optional(&mut *tx_db)
            .await
            .expect("the upload holds the item's row");
    assert_eq!(locked, Some(item));
    tx_db.rollback().await.unwrap();
    tx.send(Bytes::copy_from_slice(&whole[10..])).unwrap();
    drop(tx);
    assert_eq!(uploading.await.unwrap().0, 200);
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

#[tokio::test]
async fn a_client_that_leaves_once_its_video_arrived_cannot_cut_the_store_short() {
    let Some(node) = node().await else { return };
    let video = mp4("left");
    let (collection, item) = smart_wearable(&node, &video).await;
    let mut holder = node.pool.begin().await.unwrap();
    let holder_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    sqlx::query("SELECT 1 FROM builder.items WHERE id = $1 FOR UPDATE")
        .bind(item)
        .execute(&mut *holder)
        .await
        .unwrap();
    let uploading = tokio::spawn({
        let app = node.app.clone();
        let body = Body::from(video_form(&video));
        async move {
            send(
                &app,
                Some(OWNER_KEY),
                &format!("/v1/items/{item}/videos"),
                body,
            )
            .await
        }
    });
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let waiting: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid))",
            )
            .bind(holder_pid)
            .fetch_one(&node.pool)
            .await
            .unwrap();
            if waiting > 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the store waits on the item's row");
    uploading.abort();
    let _ = uploading.await;
    holder.rollback().await.unwrap();
    until("the store's end", || node.state.videos.idle()).await;
    assert_eq!(
        records(&node.pool, &hash(&video)).await,
        vec![(video.len() as i64, address(OWNER_KEY))]
    );
    assert!(stored(&node, &hash(&video)).await);
    assert_eq!(spool_entries(&node), 0);
    cleanup(&node.pool, &[collection], &[hash(&video)]).await;
}

const BOUNDED_KEY: &str = "0x8505050505050505050505050505050505050505050505050505050505050505";
const KEPT_COUNT_KEY: &str = "0x8606060606060606060606060606060606060606060606060606060606060606";

async fn declared(node: &Node, key: &str, path: &str, length: usize, body: Body) -> (u16, Value) {
    let mut request = post(Some(key), path, None, body).await;
    request
        .headers_mut()
        .insert(header::CONTENT_LENGTH, length.into());
    let response = tokio::time::timeout(Duration::from_secs(60), node.app.clone().oneshot(request))
        .await
        .expect("the node answers within a minute")
        .unwrap();
    let (status, body, _) = answer(response).await;
    (status, body)
}

async fn bounded_wearable(node: &Node, key: &str, video: &[u8]) -> (Uuid, Uuid) {
    let owner = address(key);
    let collection = Uuid::new_v4();
    save_collection(
        &node.state.items,
        &owner,
        json!({ "id": collection, "name": format!("Bounded {}", &collection.simple().to_string()[..12]) }),
    )
    .await
    .unwrap();
    let item = Uuid::new_v4();
    saved_item(
        &node.state.items,
        &owner,
        &json!({
            "id": item, "name": "Smart hat", "collection_id": collection, "rarity": "common",
            "video": hash(video),
            "contents": { "model.glb": hash(&unique("model")), VIDEO_FILE: hash(video) },
        }),
    )
    .await;
    (collection, item)
}

#[tokio::test]
async fn one_address_sends_at_most_its_daily_upload_bytes_whatever_its_wallets() {
    let Some(node) = node_with(
        |state| {
            state.limits.source_upload_mib =
                crate::limits::budget(2, std::time::Duration::from_secs(86_400));
        },
        true,
    )
    .await
    else {
        return;
    };
    let mut collections = Vec::new();
    let mut hashes = Vec::new();
    for (n, (key, ip, expected)) in [
        (WALLET_KEYS[0], "198.51.100.20", 200),
        (WALLET_KEYS[1], "198.51.100.20", 200),
        (WALLET_KEYS[2], "198.51.100.20", 429),
        (WALLET_KEYS[2], "198.51.100.21", 200),
    ]
    .into_iter()
    .enumerate()
    {
        let mut video = mp4_of(900 * 1024);
        video[900 * 1024 - 1] = n as u8;
        let collection = smart_wearable_of(&node, key, &video).await;
        let item = item_of(&node, collection).await;
        let form = video_form(&video);
        let mut request = post(
            Some(key),
            &format!("/v1/items/{item}/videos"),
            Some(ip),
            Body::from(form.clone()),
        )
        .await;
        request
            .headers_mut()
            .insert(header::CONTENT_LENGTH, form.len().into());
        let (status, body, retry) = answer(node.app.clone().oneshot(request).await.unwrap()).await;
        assert_eq!(status, expected, "upload {n} from {ip}: {body}");
        if expected == 429 {
            assert!(retry.is_some(), "a refusal says when to come back");
        }
        collections.push(collection);
        hashes.push(hash(&video));
    }
    cleanup(&node.pool, &collections, &hashes).await;
}

#[tokio::test]
async fn uploads_within_the_minute_a_count_answers_still_count_toward_the_bound() {
    super::validator::set_clock(chrono::Utc::now());
    let Some(node) = node_with(
        |state| {
            state.limits.wallet_bytes = 4_000;
            state.now = super::validator::clock;
        },
        true,
    )
    .await
    else {
        return;
    };
    let videos = |item: Uuid| format!("/v1/items/{item}/videos");
    let mut published = mp4_of(3_000);
    published[2_999] = 9;
    let (published_collection, item) = bounded_wearable(&node, KEPT_COUNT_KEY, &published).await;
    let form = video_form(&published);
    assert_eq!(
        declared(
            &node,
            KEPT_COUNT_KEY,
            &videos(item),
            form.len(),
            Body::from(form)
        )
        .await
        .0,
        200
    );
    let contract = on_chain(&node.pool, published_collection, false).await;
    for (n, expected) in [(1u8, 200), (2, 200), (3, 200), (4, 413)] {
        let mut video = mp4_of(1_000);
        video[999] = n;
        let (_, item) = bounded_wearable(&node, KEPT_COUNT_KEY, &video).await;
        let form = video_form(&video);
        assert_eq!(
            declared(
                &node,
                KEPT_COUNT_KEY,
                &videos(item),
                form.len(),
                Body::from(form)
            )
            .await
            .0,
            expected,
            "draft video {n}"
        );
    }
    super::support::off_chain(&node.pool, &contract).await;
}

#[tokio::test]
async fn a_wallet_at_its_storage_bound_is_refused_the_next_upload_until_it_deletes_or_publishes() {
    super::validator::set_clock(chrono::Utc::now());
    let Some(node) = node_with(
        |state| {
            state.limits.wallet_bytes = 4_000;
            state.now = super::validator::clock;
        },
        true,
    )
    .await
    else {
        return;
    };
    let [first, second, third] = [1u8, 2, 3].map(|n| {
        let mut video = mp4_of(3_000);
        video[2_999] = n;
        video
    });
    let (_, a) = bounded_wearable(&node, BOUNDED_KEY, &first).await;
    let (b_collection, b) = bounded_wearable(&node, BOUNDED_KEY, &second).await;
    let (_, c) = bounded_wearable(&node, BOUNDED_KEY, &third).await;
    let videos = |item: Uuid| format!("/v1/items/{item}/videos");
    let refused = json!({
        "ok": false,
        "data": { "maxBytes": 4_000 },
        "error": "The wallet's unpublished files reached their limit"
    });

    let form = video_form(&first);
    assert_eq!(
        declared(&node, BOUNDED_KEY, &videos(a), form.len(), Body::from(form))
            .await
            .0,
        200
    );

    for path in [videos(b), format!("/builder-api/v1/items/{b}/files")] {
        let (body, polled) = untouchable();
        assert_eq!(
            declared(&node, BOUNDED_KEY, &path, 1_200, body).await,
            (413, refused.clone()),
            "{path}"
        );
        assert_eq!(
            polled.load(Ordering::SeqCst),
            0,
            "{path}: the body was read"
        );
    }

    let mut request = Request::builder()
        .method("DELETE")
        .uri(format!("/v1/items/{a}"));
    for (name, value) in signed_map(
        BOUNDED_KEY,
        "delete",
        &format!("/v1/items/{a}"),
        "{}",
        false,
    )
    .await
    {
        request = request.header(name, value);
    }
    let deleted = answer(
        node.app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(deleted.0, 200, "{:?}", deleted.1);
    let form = video_form(&second);
    assert_eq!(
        declared(&node, BOUNDED_KEY, &videos(b), form.len(), Body::from(form))
            .await
            .0,
        200
    );

    let small = mp4("published elsewhere");
    let (d_collection, d) = bounded_wearable(&node, BOUNDED_KEY, &small).await;
    let form = video_form(&small);
    assert_eq!(
        declared(&node, BOUNDED_KEY, &videos(d), form.len(), Body::from(form))
            .await
            .0,
        200
    );
    let elsewhere = on_chain(&node.pool, d_collection, false).await;

    let form = video_form(&third);
    let (body, _) = untouchable();
    assert_eq!(
        declared(&node, BOUNDED_KEY, &videos(c), form.len(), body)
            .await
            .0,
        413,
        "another item's publication frees nothing of this one's"
    );
    let contract = on_chain(&node.pool, b_collection, false).await;
    let (body, polled) = untouchable();
    assert_eq!(
        declared(&node, BOUNDED_KEY, &videos(c), form.len(), body)
            .await
            .0,
        413,
        "the wallet's count answers for a minute, whatever the rate of its uploads"
    );
    assert_eq!(polled.load(Ordering::SeqCst), 0);
    super::validator::advance(chrono::Duration::seconds(61));
    assert_eq!(
        declared(&node, BOUNDED_KEY, &videos(c), form.len(), Body::from(form))
            .await
            .0,
        200,
        "a published item's files no longer count once the minute is over"
    );
    super::support::off_chain(&node.pool, &contract).await;
    super::support::off_chain(&node.pool, &elsewhere).await;
}

async fn video_missing_for_publication(node: &Node, collection: Uuid) -> bool {
    match node
        .state
        .items
        .prepare_publication(&crate::access::granted::publisher(
            &address(OWNER_KEY),
            collection,
        ))
        .await
    {
        Ok(_) => false,
        Err(refused) => refused.to_string().contains("upload missing content"),
    }
}

/// A smart wearable's video counts as uploaded for publication only when the
/// videos route stored it: the same bytes through `/files` under a second
/// name of the item, or through a sibling item's files, leave it missing;
/// an MP4 sent to the videos route makes it uploaded.
#[tokio::test]
async fn only_a_video_the_videos_route_took_counts_as_uploaded_for_publication() {
    let Some(node) = node().await else { return };
    let model = unique("model");
    let (collection, item) = smart_wearable(&node, &model).await;
    let save = async |id: Uuid, contents: Value| {
        let mut fields = json!({
            "id": id, "name": "Smart hat", "collection_id": collection, "rarity": "common",
            "contents": contents,
        });
        if let Some(video) = fields["contents"].get(VIDEO_FILE).cloned() {
            fields["video"] = video;
        }
        saved_item(&node.state.items, &address(OWNER_KEY), &fields).await;
    };
    let files = async |id: Uuid, parts: &[&[u8]]| {
        let parts: Vec<(String, &[u8])> = parts.iter().map(|p| (hash(p), *p)).collect();
        let parts: Vec<(&str, Option<&str>, &[u8])> = parts
            .iter()
            .map(|(h, p)| (h.as_str(), Some("blob"), *p))
            .collect();
        send(
            &node.app,
            Some(OWNER_KEY),
            &format!("/v1/items/{id}/files"),
            Body::from(form(&parts)),
        )
        .await
        .0
    };
    let page = b"<!DOCTYPE html><p>not a video</p>".to_vec();
    save(
        item,
        json!({ "model.glb": hash(&model), VIDEO_FILE: hash(&page), "extra.bin": hash(&page) }),
    )
    .await;
    assert_eq!(files(item, &[&page, &model]).await, 200);
    assert!(video_missing_for_publication(&node, collection).await);

    let other_page = b"<!DOCTYPE html><p>another non-video</p>".to_vec();
    save(
        item,
        json!({ "model.glb": hash(&model), VIDEO_FILE: hash(&other_page) }),
    )
    .await;
    let sibling = Uuid::new_v4();
    save(
        sibling,
        json!({ "model.glb": hash(&model), "skin.png": hash(&other_page) }),
    )
    .await;
    assert_eq!(files(sibling, &[&other_page]).await, 200);
    assert!(video_missing_for_publication(&node, collection).await);

    let video = mp4("checked preview");
    save(
        item,
        json!({ "model.glb": hash(&model), VIDEO_FILE: hash(&video) }),
    )
    .await;
    save(sibling, json!({ "model.glb": hash(&model) })).await;
    let (status, body) = upload(&node, Some(OWNER_KEY), item, video_form(&video)).await;
    assert_eq!(status, 200, "{body}");
    assert!(!video_missing_for_publication(&node, collection).await);
    cleanup(
        &node.pool,
        &[collection],
        &[hash(&page), hash(&other_page), hash(&video), hash(&model)],
    )
    .await;
}

/// A stranger behind a creator's address who declares large uploads and
/// sends nothing spends none of the address's day: the creator's upload from
/// it is taken.
#[tokio::test]
async fn uploads_that_send_nothing_spend_none_of_their_addresss_day() {
    let Some(node) = node_with(
        |state| {
            state.limits.source_upload_mib =
                crate::limits::budget(1_000, std::time::Duration::from_secs(86_400));
        },
        true,
    )
    .await
    else {
        return;
    };
    let ip = "198.51.100.40";
    let stranger_video = mp4("stranger");
    let stranger = smart_wearable_of(&node, WALLET_KEYS[3], &stranger_video).await;
    let stranger_item = item_of(&node, stranger).await;
    let mut statuses = Vec::new();
    for (route, declared) in [("videos", 200usize), ("files", 20)].repeat(5) {
        let mut request = post(
            Some(WALLET_KEYS[3]),
            &format!("/v1/items/{stranger_item}/{route}"),
            Some(ip),
            Body::empty(),
        )
        .await;
        request
            .headers_mut()
            .insert(header::CONTENT_LENGTH, (declared * 1024 * 1024).into());
        statuses.push(
            answer(node.app.clone().oneshot(request).await.unwrap())
                .await
                .0,
        );
    }
    assert!(statuses.iter().all(|s| *s == 400), "{statuses:?}");
    let video = mp4("creator behind the same address");
    let creator = smart_wearable_of(&node, WALLET_KEYS[4], &video).await;
    let item = item_of(&node, creator).await;
    let form = video_form(&video);
    let mut request = post(
        Some(WALLET_KEYS[4]),
        &format!("/v1/items/{item}/videos"),
        Some(ip),
        Body::from(form.clone()),
    )
    .await;
    request
        .headers_mut()
        .insert(header::CONTENT_LENGTH, form.len().into());
    let (status, body, retry) = answer(node.app.clone().oneshot(request).await.unwrap()).await;
    cleanup(&node.pool, &[stranger, creator], &[hash(&video)]).await;
    assert_eq!((status, retry), (200, None), "{body}");
}

/// Files an address really sent count toward its day like videos: past it,
/// the next upload from the address waits.
#[tokio::test]
async fn files_an_address_sent_count_toward_its_day() {
    let Some(node) = node_with(
        |state| {
            state.limits.source_upload_mib =
                crate::limits::budget(2, std::time::Duration::from_secs(86_400));
        },
        true,
    )
    .await
    else {
        return;
    };
    let ip = "198.51.100.41";
    let mut collections = Vec::new();
    let mut statuses = Vec::new();
    for n in 0..2u8 {
        let mut big = vec![n; 1_500_000];
        big[0] = 7;
        let (collection, item) = bounded_wearable(&node, WALLET_KEYS[0], &mp4("unused")).await;
        saved_item(
            &node.state.items,
            &address(WALLET_KEYS[0]),
            &json!({ "id": item, "name": "Big hat", "collection_id": collection, "rarity": "common",
                     "contents": { "model.glb": hash(&big) } }),
        )
        .await;
        let request = post(
            Some(WALLET_KEYS[0]),
            &format!("/v1/items/{item}/files"),
            Some(ip),
            Body::from(form(&[(&hash(&big), Some("blob"), &big)])),
        )
        .await;
        statuses.push(
            answer(node.app.clone().oneshot(request).await.unwrap())
                .await
                .0,
        );
        collections.push(collection);
    }
    cleanup(&node.pool, &collections, &[]).await;
    assert_eq!(statuses, [200, 429]);
}

/// The end of `form`'s first part headers.
fn headers_end(form: &[u8]) -> usize {
    form.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4
}

/// The owner's upload of `item`'s `video`, trickling: its task, the body's
/// sender and the bytes sent, once the node has read the part headers and
/// 40 bytes of the video; then `windows` steps of 20 paused seconds, one
/// more byte each. With `declare_length` its request carries the form's
/// `Content-Length`.
async fn trickling(
    node: &Node,
    item: Uuid,
    video: &[u8],
    windows: usize,
    declare_length: bool,
) -> (
    tokio::task::JoinHandle<(u16, Value, Option<String>)>,
    tokio::sync::mpsc::UnboundedSender<Bytes>,
    usize,
) {
    let (body, tx, pulled) = fed();
    let whole = video_form(video);
    let length = declare_length.then_some(whole.len());
    let uploading = tokio::spawn({
        let app = node.app.clone();
        async move {
            let path = format!("/v1/items/{item}/videos");
            let mut request = post(Some(OWNER_KEY), &path, None, body).await;
            if let Some(length) = length {
                request
                    .headers_mut()
                    .insert(header::CONTENT_LENGTH, length.into());
            }
            answer(app.oneshot(request).await.unwrap()).await
        }
    });
    let mut sent = headers_end(&whole) + 40;
    tx.send(Bytes::copy_from_slice(&whole[..sent])).unwrap();
    until("the upload's spool file", || spool_entries(node) == 1).await;
    pulled_to(&pulled, sent, || uploading.is_finished()).await;
    tokio::time::pause();
    for _ in 0..windows {
        tokio::time::advance(Duration::from_secs(20)).await;
        tx.send(Bytes::copy_from_slice(&whole[sent..sent + 1]))
            .unwrap();
        sent += 1;
        pulled_to(&pulled, sent, || uploading.is_finished()).await;
    }
    tokio::time::resume();
    (uploading, tx, sent)
}

/// A one-place node held by the owner's trickling video.
struct Held {
    node: Node,
    collection: Uuid,
    uploading: tokio::task::JoinHandle<(u16, Value, Option<String>)>,
    tx: tokio::sync::mpsc::UnboundedSender<Bytes>,
    sent: usize,
    video: Vec<u8>,
}

impl Held {
    /// The held upload's answer once the rest of its body arrives.
    async fn finish(self) -> (Node, Uuid, Vec<u8>, (u16, Value, Option<String>)) {
        let whole = video_form(&self.video);
        let _ = self.tx.send(Bytes::copy_from_slice(&whole[self.sent..]));
        drop(self.tx);
        let answered = tokio::time::timeout(Duration::from_secs(10), self.uploading)
            .await
            .expect("the held upload ended")
            .unwrap();
        (self.node, self.collection, self.video, answered)
    }
}

/// A one-place node, `configure`d, held by the owner's trickling video, the
/// owner having finished `spent` seconds of uploads today; with
/// `declare_length` the video's request carries its `Content-Length`.
async fn one_place_held(
    configure: impl FnOnce(&mut crate::State),
    spent: u32,
    declare_length: bool,
) -> Option<Held> {
    let node = node_with(
        |state| {
            state.videos.places = crate::items::places::Places::new("video uploads", 1, 1, 2);
            configure(state);
        },
        true,
    )
    .await?;
    node.state
        .videos
        .used
        .wallets
        .charge_at(&address(OWNER_KEY), spent, std::time::Instant::now());
    let video = mp4("held by a creator");
    let (collection, item) = smart_wearable(&node, &video).await;
    let (uploading, tx, sent) = trickling(&node, item, &video, 2, declare_length).await;
    Some(Held {
        node,
        collection,
        uploading,
        tx,
        sent,
        video,
    })
}

/// `key`'s upload of a new smart wearable's video from `ip`, its body the
/// form's part headers and then nothing: its task and collection, once the
/// node reads its body.
async fn silent_newcomer(
    node: &Node,
    key: &str,
    ip: &str,
) -> (tokio::task::JoinHandle<(u16, Value, Option<String>)>, Uuid) {
    let video = mp4("a newcomer that sends nothing");
    let collection = smart_wearable_of(node, key, &video).await;
    let item = item_of(node, collection).await;
    let whole = video_form(&video);
    let (body, pulled) = counted(whole[..headers_end(&whole)].to_vec(), 0, vec![], true);
    let app = node.app.clone();
    let (key, ip) = (key.to_string(), ip.to_string());
    let task = tokio::spawn(async move {
        let request = post(
            Some(&key),
            &format!("/v1/items/{item}/videos"),
            Some(&ip),
            body,
        )
        .await;
        answer(app.oneshot(request).await.unwrap()).await
    });
    until("the newcomer's place", || {
        pulled.load(Ordering::SeqCst) > 0 || task.is_finished()
    })
    .await;
    (task, collection)
}

/// A creator who finished a video today is trickling another while the
/// node's one place is held: a fresh wallet that takes a place and then
/// sends nothing cuts nothing, and the creator's upload finishes.
#[tokio::test]
async fn a_newcomer_that_sends_nothing_cuts_no_running_upload() {
    let Some(held) = one_place_held(|_| {}, 600, false).await else {
        return;
    };
    let (newcomer, other) = silent_newcomer(&held.node, WALLET_KEYS[0], "198.51.100.50").await;
    settle().await;
    assert!(
        !held.uploading.is_finished(),
        "the creator's upload was cut"
    );
    let (node, collection, video, (status, answer, _)) = held.finish().await;
    assert_eq!(status, 200, "{answer}");
    newcomer.abort();
    let _ = newcomer.await;
    until("every place back", || node.state.videos.idle()).await;
    cleanup(&node.pool, &[collection, other], &[hash(&video)]).await;
}

/// A fresh wallet that takes a place, sends one byte of its video and
/// stalls cuts nothing: the creator's upload behind the pace finishes.
#[tokio::test]
async fn a_newcomer_that_sends_one_byte_and_stalls_cuts_no_running_upload() {
    let Some(held) = one_place_held(|_| {}, 0, false).await else {
        return;
    };
    let node = &held.node;
    let video = mp4("a newcomer that sends a byte");
    let other = smart_wearable_of(node, WALLET_KEYS[0], &video).await;
    let item = item_of(node, other).await;
    let whole = video_form(&video);
    let head = whole[..headers_end(&whole) + 1].to_vec();
    let (body, pulled) = counted(head.clone(), 0, vec![], true);
    let newcomer = tokio::spawn({
        let app = node.app.clone();
        async move {
            let path = format!("/v1/items/{item}/videos");
            let request = post(Some(WALLET_KEYS[0]), &path, Some("198.51.100.50"), body).await;
            answer(app.oneshot(request).await.unwrap()).await
        }
    });
    until("the newcomer's byte", || {
        pulled.load(Ordering::SeqCst) >= head.len() as u64 || newcomer.is_finished()
    })
    .await;
    settle().await;
    assert!(
        !held.uploading.is_finished(),
        "the creator's upload was cut"
    );
    let (node, collection, video, (status, answer, _)) = held.finish().await;
    assert_eq!(status, 200, "{answer}");
    newcomer.abort();
    let _ = newcomer.await;
    until("every place back", || node.state.videos.idle()).await;
    cleanup(&node.pool, &[collection, other], &[hash(&video)]).await;
}

/// An upload behind the pace the largest video needs gives its place to a
/// newcomer once the newcomer has sent what that pace brings in one idle
/// bound: the newcomer's video is kept, the slow one answers 408 with
/// `Retry-After` and leaves no spool file.
#[tokio::test]
async fn a_slow_upload_gives_its_place_to_a_newcomer_that_sends() {
    let Some(Held {
        node,
        collection,
        uploading,
        tx,
        ..
    }) = one_place_held(|_| {}, 0, false).await
    else {
        return;
    };
    let (status, error, _, other) =
        another_upload_of(&node, WALLET_KEYS[0], "198.51.100.50", &mp4_of(EARNS_A_CUT)).await;
    assert_eq!(status, 200, "{error:?}");
    let (status, answer, retry) = tokio::time::timeout(Duration::from_secs(10), uploading)
        .await
        .expect("the slow upload ended")
        .unwrap();
    assert_eq!(
        (status, answer["error"].as_str(), retry.as_deref()),
        (
            408,
            Some("The upload gave its place to another while the node was busy"),
            Some("30")
        )
    );
    drop(tx);
    until("every place back", || node.state.videos.idle()).await;
    assert_eq!(spool_entries(&node), 0, "the cut upload's spool file stays");
    cleanup(&node.pool, &[collection, other], &[]).await;
}

/// An upload below the pace whose declared length and rate show it
/// finishes by its deadline keeps its place: a newcomer that earned a place
/// finds no upload behind and is refused with 429 and `Retry-After`, and the
/// creator's video is kept.
#[tokio::test]
async fn an_upload_on_track_to_finish_by_its_deadline_keeps_its_place() {
    let Some(held) = one_place_held(|_| {}, 0, true).await else {
        return;
    };
    let (status, error, retry, other) = another_upload_of(
        &held.node,
        WALLET_KEYS[0],
        "198.51.100.50",
        &mp4_of(EARNS_A_CUT),
    )
    .await;
    assert_eq!(
        (status, error.as_deref(), retry.as_deref()),
        (
            429,
            Some("Too many video uploads in progress, try again later"),
            Some("30")
        )
    );
    let (node, collection, video, (status, answer, _)) = held.finish().await;
    assert_eq!(status, 200, "{answer}");
    until("every place back", || node.state.videos.idle()).await;
    cleanup(&node.pool, &[collection, other], &[hash(&video)]).await;
}

/// An upload keeping the pace its route needs keeps its place, whatever its
/// wallet uploaded today: a newcomer is refused at once with 429.
#[tokio::test]
async fn an_upload_at_its_pace_keeps_its_place() {
    let Some(held) = one_place_held(
        |state| state.videos.deadline = Duration::from_secs(1_000_000_000),
        600,
        false,
    )
    .await
    else {
        return;
    };
    let (status, error, retry, other) =
        another_upload(&held.node, WALLET_KEYS[0], "198.51.100.50").await;
    assert_eq!(
        (status, error.as_deref(), retry.as_deref()),
        (
            429,
            Some("Too many video uploads in progress, try again later"),
            Some("30")
        )
    );
    let (node, collection, video, (status, answer, _)) = held.finish().await;
    assert_eq!(status, 200, "{answer}");
    until("every place back", || node.state.videos.idle()).await;
    cleanup(&node.pool, &[collection, other], &[hash(&video)]).await;
}

/// A newcomer its budgets refuse cuts no upload, even one behind its pace.
#[tokio::test]
async fn a_newcomer_past_its_budgets_cuts_no_upload() {
    let Some(held) = one_place_held(|_| {}, 600, false).await else {
        return;
    };
    let ip = "198.51.100.51";
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("x-real-ip", ip.parse().unwrap());
    let site = crate::limits::upload_source(&headers).unwrap();
    held.node
        .state
        .limits
        .source_upload_mib
        .charge_at(&site, u32::MAX, std::time::Instant::now());
    let (status, _, _, other) = another_upload(&held.node, WALLET_KEYS[0], ip).await;
    assert_eq!(status, 429);
    settle().await;
    assert!(
        !held.uploading.is_finished(),
        "the creator's upload was cut"
    );
    let (node, collection, video, (status, answer, _)) = held.finish().await;
    assert_eq!(status, 200, "{answer}");
    until("every place back", || node.state.videos.idle()).await;
    cleanup(&node.pool, &[collection, other], &[hash(&video)]).await;
}

/// A new draft of `key`'s naming `n` tiny files, uploaded through `/files`
/// in one request from `ip`: the status and the draft's collection.
async fn tiny_files(node: &Node, key: &str, ip: &str, n: usize, tag: &str) -> (u16, Value, Uuid) {
    let files: Vec<Vec<u8>> = (0..n).map(|i| format!("{tag} {i}").into_bytes()).collect();
    let (collection, item) = bounded_wearable(node, key, &mp4("unused")).await;
    let contents: serde_json::Map<String, Value> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (format!("f{i}.bin"), json!(hash(f))))
        .collect();
    saved_item(
        &node.state.items,
        &address(key),
        &json!({ "id": item, "name": "Tiny hat", "collection_id": collection, "rarity": "common",
                 "contents": contents }),
    )
    .await;
    let hashes: Vec<String> = files.iter().map(|f| hash(f)).collect();
    let parts: Vec<(&str, Option<&str>, &[u8])> = hashes
        .iter()
        .zip(&files)
        .map(|(h, f)| (h.as_str(), Some("blob"), f.as_slice()))
        .collect();
    let request = post(
        Some(key),
        &format!("/v1/items/{item}/files"),
        Some(ip),
        Body::from(form(&parts)),
    )
    .await;
    let (status, body, _) = answer(node.app.clone().oneshot(request).await.unwrap()).await;
    (status, body, collection)
}

const FILE_COUNT_KEYS: [&str; 2] = [
    "0x8611111111111111111111111111111111111111111111111111111111111111",
    "0x8622222222222222222222222222222222222222222222222222222222222222",
];

/// A wallet's unpublished files are bounded in number as in bytes, so tiny
/// files can't pass the byte bound by their count; other wallets upload on.
#[tokio::test]
async fn a_wallet_holds_at_most_its_bound_of_unpublished_files() {
    let Some(node) = node_with(|state| state.limits.wallet_files = 3, true).await else {
        return;
    };
    let (status, _, a) = tiny_files(&node, FILE_COUNT_KEYS[0], "198.51.100.60", 3, "first").await;
    assert_eq!(status, 200);
    let (status, body, b) =
        tiny_files(&node, FILE_COUNT_KEYS[0], "198.51.100.60", 1, "second").await;
    assert_eq!(
        (status, &body["data"]["maxFiles"]),
        (413, &json!(3)),
        "{body}"
    );
    let (status, _, c) = tiny_files(&node, FILE_COUNT_KEYS[1], "198.51.100.60", 1, "other").await;
    assert_eq!(status, 200);
    cleanup(&node.pool, &[a, b, c], &[]).await;
}

/// The node holds at most its bound of unpublished files and bytes, however
/// many wallets share them: past either, every upload is refused.
#[tokio::test]
async fn the_node_holds_at_most_its_bound_of_unpublished_files_and_bytes() {
    let Some(node) = isolated_node(|state| state.limits.node_files = 4).await else {
        return;
    };
    let (status, _, a) = tiny_files(&node, WALLET_KEYS[0], "198.51.100.61", 3, "first").await;
    assert_eq!(status, 200);
    let (status, _, b) = tiny_files(&node, WALLET_KEYS[1], "198.51.100.62", 1, "second").await;
    assert_eq!(status, 200);
    let (status, body, c) = tiny_files(&node, WALLET_KEYS[2], "198.51.100.63", 1, "third").await;
    assert_eq!(
        (status, &body["data"]["maxFiles"]),
        (507, &json!(4)),
        "{body}"
    );
    cleanup(&node.pool, &[a, b, c], &[]).await;

    let Some(node) = isolated_node(|state| state.limits.node_bytes = 4_000).await else {
        return;
    };
    let mut collections = Vec::new();
    for (n, (key, expected)) in [(WALLET_KEYS[0], 200), (WALLET_KEYS[1], 507)]
        .into_iter()
        .enumerate()
    {
        let mut video = mp4_of(3_000);
        video[2_999] = n as u8;
        let (collection, item) = bounded_wearable(&node, key, &video).await;
        let form = video_form(&video);
        let (status, body) = declared(
            &node,
            key,
            &format!("/v1/items/{item}/videos"),
            form.len(),
            Body::from(form),
        )
        .await;
        assert_eq!(status, expected, "{body}");
        if expected == 507 {
            assert_eq!(body["data"]["maxBytes"], json!(4_000));
        }
        collections.push(collection);
    }
    cleanup(&node.pool, &collections, &[]).await;
}

/// A new draft of `key`'s naming `files`: its collection and item.
async fn draft_naming(node: &Node, key: &str, files: &[&[u8]]) -> (Uuid, Uuid) {
    let (collection, item) = bounded_wearable(node, key, &mp4("unused")).await;
    let contents: serde_json::Map<String, Value> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (format!("f{i}.bin"), json!(hash(f))))
        .collect();
    saved_item(
        &node.state.items,
        &address(key),
        &json!({ "id": item, "name": "Tiny hat", "collection_id": collection, "rarity": "common",
                 "contents": contents }),
    )
    .await;
    (collection, item)
}

/// `key`'s upload of `files` to `item` from `ip`, each its own part: the
/// status and body.
async fn send_files(node: &Node, key: &str, ip: &str, item: Uuid, files: &[&[u8]]) -> (u16, Value) {
    let hashes: Vec<String> = files.iter().map(|f| hash(f)).collect();
    let parts: Vec<(&str, Option<&str>, &[u8])> = hashes
        .iter()
        .zip(files)
        .map(|(h, f)| (h.as_str(), Some("blob"), *f))
        .collect();
    let (status, body, _) = send_from(
        &node.app,
        Some(key),
        &format!("/v1/items/{item}/files"),
        Some(ip),
        Body::from(form(&parts)),
    )
    .await;
    (status, body)
}

/// Files sent again, by their uploader or twice in one form, add no record,
/// so they take none of the node's room: another wallet uploads on. A file
/// twice in one form costs its address's day one file.
#[tokio::test]
async fn files_sent_again_take_none_of_the_nodes_room() {
    let Some(node) = isolated_node(|state| state.limits.node_files = 4).await else {
        return;
    };
    let files: [&[u8]; 3] = [b"sent again one", b"sent again two", b"sent again three"];
    let (first, item) = draft_naming(&node, WALLET_KEYS[0], &files).await;
    let ip = "198.51.100.64";
    let twice = [files[0], files[0], files[1], files[2]];
    assert_eq!(
        send_files(&node, WALLET_KEYS[0], ip, item, &twice).await.0,
        200
    );
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("x-real-ip", ip.parse().unwrap());
    let site = crate::limits::upload_source(&headers).unwrap();
    let spent = node
        .state
        .limits
        .source_upload_files
        .spent_at(&site, std::time::Instant::now());
    assert!(spent < 3.5, "the address paid {spent} files for 3");
    assert_eq!(
        send_files(&node, WALLET_KEYS[0], ip, item, &files).await.0,
        200
    );
    let other: [&[u8]; 1] = [b"another wallet's file"];
    let (second, item) = draft_naming(&node, WALLET_KEYS[1], &other).await;
    let (status, body) = send_files(&node, WALLET_KEYS[1], "198.51.100.65", item, &other).await;
    assert_eq!(status, 200, "{body}");
    cleanup(&node.pool, &[first, second], &[]).await;
}

const DAY_KEYS: [&str; 4] = [
    "0x8811111111111111111111111111111111111111111111111111111111111111",
    "0x8822222222222222222222222222222222222222222222222222222222222222",
    "0x8833333333333333333333333333333333333333333333333333333333333333",
    "0x8844444444444444444444444444444444444444444444444444444444444444",
];

fn files_a_day(files: u32) -> crate::shared::limits::RateLimiter {
    crate::limits::budget(files, Duration::from_secs(86_400))
}

/// Tiny files cost an address its day of files however few bytes they
/// carry: two wallets behind one address send 512 tiny files, then the
/// address waits, while another address uploads on.
#[tokio::test]
async fn an_address_uploads_at_most_its_day_of_files_however_small() {
    let Some(node) = node_with(
        |state| state.limits.source_upload_files = files_a_day(300),
        true,
    )
    .await
    else {
        return;
    };
    let ip = "198.51.100.80";
    let (a, _, ca) = tiny_files(&node, DAY_KEYS[0], ip, 256, "day a").await;
    let (b, _, cb) = tiny_files(&node, DAY_KEYS[1], ip, 256, "day b").await;
    let (c, body, cc) = tiny_files(&node, DAY_KEYS[2], ip, 1, "day c").await;
    let (d, _, cd) = tiny_files(&node, DAY_KEYS[2], "198.51.100.81", 1, "day d").await;
    cleanup(&node.pool, &[ca, cb, cc, cd], &[]).await;
    assert_eq!((a, b, c, d), (200, 200, 429, 200), "{body}");
}

/// The IPv6 prefixes of one site share the site's day of files: a /48
/// counts as one address, so new /64s under it buy no more.
#[tokio::test]
async fn the_prefixes_of_one_site_share_its_day_of_files() {
    let Some(node) = node_with(
        |state| state.limits.source_upload_files = files_a_day(300),
        true,
    )
    .await
    else {
        return;
    };
    let (a, _, ca) = tiny_files(&node, DAY_KEYS[0], "2001:db8:5:1::1", 256, "site a").await;
    let (b, _, cb) = tiny_files(&node, DAY_KEYS[1], "2001:db8:5:2::1", 256, "site b").await;
    let (c, body, cc) = tiny_files(&node, DAY_KEYS[2], "2001:db8:5:3::1", 1, "site c").await;
    let (d, _, cd) = tiny_files(&node, DAY_KEYS[2], "2001:db8:6:1::1", 1, "site d").await;
    cleanup(&node.pool, &[ca, cb, cc, cd], &[]).await;
    assert_eq!((a, b, c, d), (200, 200, 429, 200), "{body}");
}

/// A wallet's day of files follows it from address to address.
#[tokio::test]
async fn a_wallet_uploads_at_most_its_day_of_files_from_any_address() {
    let Some(node) = node_with(
        |state| state.limits.wallet_upload_files = files_a_day(300),
        true,
    )
    .await
    else {
        return;
    };
    let (a, _, ca) = tiny_files(&node, DAY_KEYS[3], "198.51.100.82", 256, "wallet a").await;
    let (b, _, cb) = tiny_files(&node, DAY_KEYS[3], "198.51.100.83", 256, "wallet b").await;
    let (c, body, cc) = tiny_files(&node, DAY_KEYS[3], "198.51.100.84", 1, "wallet c").await;
    let (d, _, cd) = tiny_files(&node, DAY_KEYS[0], "198.51.100.84", 1, "wallet d").await;
    cleanup(&node.pool, &[ca, cb, cc, cd], &[]).await;
    assert_eq!((a, b, c, d), (200, 200, 429, 200), "{body}");
}

/// Once the node's unpublished files pass half its bound, every address
/// pays ten times for each file: what an address sent freely before now
/// spends its day, while creators below their share still upload.
#[tokio::test]
async fn past_half_its_bound_the_node_charges_each_address_ten_times_for_its_files() {
    let Some(node) = isolated_node(|state| {
        state.limits.node_files = 100;
        state.limits.source_upload_files = files_a_day(300);
    })
    .await
    else {
        return;
    };
    let (a, _, ca) = tiny_files(&node, DAY_KEYS[0], "198.51.100.85", 31, "half a").await;
    let (b, _, cb) = tiny_files(&node, DAY_KEYS[0], "198.51.100.85", 1, "half b").await;
    assert_eq!(
        (a, b),
        (200, 200),
        "32 files cost 32 of 300 below half the bound"
    );
    let (c, _, cc) = tiny_files(&node, DAY_KEYS[1], "198.51.100.86", 20, "half c").await;
    assert_eq!(c, 200);
    let (d, _, cd) = tiny_files(&node, DAY_KEYS[2], "198.51.100.87", 31, "half d").await;
    let (e, body, ce) = tiny_files(&node, DAY_KEYS[2], "198.51.100.87", 1, "half e").await;
    let (f, _, cf) = tiny_files(&node, DAY_KEYS[3], "198.51.100.88", 1, "half f").await;
    cleanup(&node.pool, &[ca, cb, cc, cd, ce, cf], &[]).await;
    assert_eq!(
        (d, e, f),
        (200, 429, 200),
        "31 files cost 310 of 300 past half the bound: {body}"
    );
}

/// Past its bound the node refuses uploads at once, however many records
/// and holders its stores hold: no request waits on a count of them all.
#[tokio::test]
async fn a_node_past_its_file_bound_refuses_uploads_without_counting_every_record_first() {
    let Some(node) = isolated_node(|state| state.limits.node_files = 50_000).await else {
        return;
    };
    sqlx::query(
        "WITH w AS (SELECT n, '0x' || lpad(to_hex(n), 40, '0') AS a, gen_random_uuid() AS c, \
           gen_random_uuid() AS i FROM generate_series(1, 20) n), \
         cs AS (INSERT INTO builder.collections (id, name, eth_address) SELECT c, 'held ' || n, a FROM w RETURNING id), \
         its AS (INSERT INTO builder.items (id, collection_id, eth_address, name, type) \
           SELECT i, c, a, 'held', 'wearable' FROM w JOIN cs ON cs.id = w.c RETURNING id), \
         ic AS (INSERT INTO builder.item_contents (item_id, file, hash, named_by) \
           SELECT w.i, 'f' || f || '.bin', 'bafkreiheld' || lpad(f::text, 48, '0'), w.a \
           FROM w JOIN its ON its.id = w.i, generate_series(1, 5000) f RETURNING 1) \
         INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) \
         SELECT 'bafkreiheld' || lpad(f::text, 48, '0'), 8, w.a FROM w, generate_series(1, 5000) f",
    )
    .execute(&node.pool)
    .await
    .unwrap();
    sqlx::query("ANALYZE").execute(&node.pool).await.unwrap();
    let started = std::time::Instant::now();
    let (first, body, _) = tiny_files(&node, DAY_KEYS[0], "198.51.100.89", 1, "full a").await;
    let (second, _, _) = tiny_files(&node, DAY_KEYS[1], "198.51.100.90", 1, "full b").await;
    let took = started.elapsed();
    assert_eq!(
        (first, &body["data"]["maxFiles"]),
        (507, &json!(50_000)),
        "{body}"
    );
    assert_eq!(second, 507);
    assert!(took < Duration::from_secs(10), "two uploads took {took:?}");
}

const DROPPED_RECORDS_KEY: &str =
    "0x8707070707070707070707070707070707070707070707070707070707070707";

/// A wallet whose published records drop while its count is kept is
/// counted from what it holds, never below nothing.
#[tokio::test]
async fn records_dropped_within_the_minute_a_count_answers_never_refuse_a_wallet_below_its_bound() {
    super::validator::set_clock(chrono::Utc::now());
    let Some(node) = node_with(
        |state| {
            state.limits.wallet_bytes = 10_000;
            state.now = super::validator::clock;
        },
        true,
    )
    .await
    else {
        return;
    };
    let videos = |item: Uuid| format!("/v1/items/{item}/videos");
    let sized = |len: usize, n: u8| {
        let mut video = mp4_of(len);
        video[len - 1] = n;
        video
    };
    let send_video = async |video: &[u8]| {
        let (collection, item) = bounded_wearable(&node, DROPPED_RECORDS_KEY, video).await;
        let form = video_form(video);
        let status = declared(
            &node,
            DROPPED_RECORDS_KEY,
            &videos(item),
            form.len(),
            Body::from(form),
        )
        .await
        .0;
        (collection, status)
    };
    let (pa, pb) = (sized(5_000, 1), sized(5_000, 2));
    let (pa_collection, status) = send_video(&pa).await;
    assert_eq!(status, 200);
    let pa_contract = on_chain(&node.pool, pa_collection, false).await;
    let (pb_collection, status) = send_video(&pb).await;
    assert_eq!(status, 200);
    let pb_contract = on_chain(&node.pool, pb_collection, false).await;
    super::validator::advance(chrono::Duration::seconds(61));
    let small = sized(200, 3);
    let (_, status) = send_video(&small).await;
    assert_eq!(
        status, 200,
        "both published videos are public: nothing unpublished"
    );
    sqlx::query("DELETE FROM builder.uploaded_contents WHERE hash = $1")
        .bind(hash(&pa))
        .execute(&node.pool)
        .await
        .unwrap();
    let unpublished = crate::access::unpublished(&node.pool, &address(DROPPED_RECORDS_KEY))
        .await
        .unwrap()
        .bytes;
    let draft = sized(5_000, 4);
    let (_, status) = send_video(&draft).await;
    super::support::off_chain(&node.pool, &pa_contract).await;
    super::support::off_chain(&node.pool, &pb_contract).await;
    assert_eq!((unpublished, status), (200, 200));
}

const DRAFT_BOUND_KEY: &str = "0x8909090909090909090909090909090909090909090909090909090909090909";

/// A wallet's published items don't count toward its bound of drafts: at
/// the bound a new draft is refused until one of its collections is
/// published.
#[tokio::test]
async fn a_wallets_published_items_leave_room_for_new_drafts() {
    let Some(node) = node_with(|state| state.items.documents.items = 2, true).await else {
        return;
    };
    let owner = address(DRAFT_BOUND_KEY);
    let (first, _) = bounded_wearable(&node, DRAFT_BOUND_KEY, &mp4("bound first")).await;
    let (second, _) = bounded_wearable(&node, DRAFT_BOUND_KEY, &mp4("bound second")).await;
    let third = || json!({ "id": Uuid::new_v4(), "name": "Third hat", "collection_id": second, "rarity": "common" });
    let refused = super::support::save_item(&node.state.items, &owner, &third())
        .await
        .err();
    let contract = on_chain(&node.pool, first, false).await;
    let saved = super::support::save_item(&node.state.items, &owner, &third())
        .await
        .map(|_| ());
    super::support::off_chain(&node.pool, &contract).await;
    cleanup(&node.pool, &[first, second], &[]).await;
    assert_eq!(refused.map(|e| e.status()), Some(413));
    assert!(saved.is_ok(), "{saved:?}");
}

/// A file upload whose body stops arriving ends at its deadline with 408
/// and gives its place back, so stalled senders can't hold the node's file
/// places.
#[tokio::test]
async fn a_stalled_file_upload_ends_at_its_deadline_and_frees_its_place() {
    let Some(node) = node_with(|state| state.file_deadline = Duration::from_secs(1), true).await
    else {
        return;
    };
    let collection = smart_wearable_of(&node, WALLET_KEYS[3], &mp4("stalled files")).await;
    let item = item_of(&node, collection).await;
    let (body, _) = counted(b"--videoboundary\r\n".to_vec(), 0, vec![], true);
    let answered = tokio::time::timeout(
        Duration::from_secs(20),
        send_from(
            &node.app,
            Some(WALLET_KEYS[3]),
            &format!("/v1/items/{item}/files"),
            Some("198.51.100.95"),
            body,
        ),
    )
    .await;
    cleanup(&node.pool, &[collection], &[]).await;
    let (status, answer, _) = answered.expect("the stalled upload was answered");
    assert_eq!(
        (status, answer["error"].as_str()),
        (408, Some("The upload timed out"))
    );
    assert_eq!(node.state.file_uploads.in_flight(), 0);
}

/// A file upload whose body stops arriving ends at the idle bound with 408,
/// long before its deadline, and gives its place back.
#[tokio::test]
async fn a_file_upload_that_stalls_ends_at_the_idle_bound() {
    let Some(node) = node().await else { return };
    let file = b"a texture that stops halfway".to_vec();
    let (collection, item) = smart_wearable(&node, &mp4("stalling files")).await;
    let whole = form(&[(&hash(&file), Some("blob"), &file)]);
    let (body, tx, pulled) = fed();
    let uploading = tokio::spawn({
        let app = node.app.clone();
        async move { send_unbounded(&app, OWNER_KEY, &format!("/v1/items/{item}/files"), body).await }
    });
    let sent = headers_end(&whole) + 8;
    tx.send(Bytes::copy_from_slice(&whole[..sent])).unwrap();
    pulled_to(&pulled, sent, || uploading.is_finished()).await;
    tokio::time::pause();
    settle().await;
    tokio::time::advance(Duration::from_secs(29)).await;
    settle().await;
    assert!(!uploading.is_finished(), "cut before the idle bound");
    tokio::time::advance(Duration::from_secs(2)).await;
    settle().await;
    tokio::time::resume();
    let (status, answer) = tokio::time::timeout(Duration::from_secs(10), uploading)
        .await
        .expect("a stalled file upload is cut at the idle bound")
        .unwrap();
    assert_eq!(
        (status, answer["error"].as_str()),
        (408, Some("The upload timed out"))
    );
    assert_eq!(node.state.file_uploads.in_flight(), 0, "the place was kept");
    drop(tx);
    cleanup(&node.pool, &[collection], &[]).await;
}

/// A file upload behind its pace gives its place to a creator who has sent
/// what that pace brings in one idle bound (21 MiB over 5 minutes, for 30
/// seconds: about 2.2 MB): the creator's files are kept and the stalled
/// upload answers 408 with `Retry-After`. A creator whose few small files
/// arrive first is served without cutting it.
#[tokio::test]
async fn a_stalled_file_upload_gives_its_place_to_a_creator_that_sends() {
    let Some(node) = node_with(
        |state| state.file_uploads = crate::items::places::Places::new("file uploads", 1, 1, 2),
        true,
    )
    .await
    else {
        return;
    };
    let collection = smart_wearable_of(&node, WALLET_KEYS[0], &mp4("stalled files")).await;
    let item = item_of(&node, collection).await;
    let (body, _) = counted(b"--videoboundary\r\n".to_vec(), 0, vec![], true);
    let stalled = tokio::spawn({
        let app = node.app.clone();
        async move {
            let path = format!("/v1/items/{item}/files");
            let request = post(Some(WALLET_KEYS[0]), &path, Some("198.51.100.80"), body).await;
            answer(app.oneshot(request).await.unwrap()).await
        }
    });
    until("the stalled upload's place", || {
        node.state.file_uploads.in_flight() == 1
    })
    .await;
    let (status, body, small) =
        tiny_files(&node, WALLET_KEYS[1], "198.51.100.81", 2, "creator").await;
    assert_eq!(status, 200, "{body}");
    settle().await;
    assert!(!stalled.is_finished(), "small files cut the stalled upload");
    let file = vec![9u8; 2_400_000];
    let (creator, item) = bounded_wearable(&node, WALLET_KEYS[2], &mp4("unused")).await;
    saved_item(
        &node.state.items,
        &address(WALLET_KEYS[2]),
        &json!({ "id": item, "name": "Large hat", "collection_id": creator, "rarity": "common",
                 "contents": { "model.glb": hash(&file) } }),
    )
    .await;
    let (status, body, _) = send_from(
        &node.app,
        Some(WALLET_KEYS[2]),
        &format!("/v1/items/{item}/files"),
        Some("198.51.100.82"),
        Body::from(form(&[(&hash(&file), Some("blob"), &file)])),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, answer, retry) = tokio::time::timeout(Duration::from_secs(10), stalled)
        .await
        .expect("the stalled upload ended")
        .unwrap();
    assert_eq!(
        (status, answer["error"].as_str(), retry.as_deref()),
        (
            408,
            Some("The upload gave its place to another while the node was busy"),
            Some("30")
        )
    );
    until("every place back", || {
        node.state.file_uploads.in_flight() == 0
    })
    .await;
    cleanup(&node.pool, &[collection, small, creator], &[hash(&file)]).await;
}

/// An upload declaring a body longer than its route takes is refused at
/// once with the route's own 413, before it takes a place or counts what the
/// node holds, even while every place is held.
#[tokio::test]
async fn an_upload_declaring_more_than_its_route_takes_is_refused_before_a_place() {
    let Some(node) = node_with(
        |state| {
            state.videos.places = crate::items::places::Places::new("video uploads", 1, 1, 2);
            state.file_uploads = crate::items::places::Places::new("file uploads", 1, 1, 2);
        },
        true,
    )
    .await
    else {
        return;
    };
    let _videos = node
        .state
        .videos
        .admit("0xsomeone", None, None)
        .ok()
        .unwrap();
    let _files = node
        .state
        .file_uploads
        .admit(
            "0xsomeone",
            None,
            crate::items::places::Timing {
                deadline: crate::items::upload::DEADLINE,
                idle: crate::items::places::IDLE_TIMEOUT,
                most: crate::UPLOAD_BODY_LIMIT as u64,
            },
            None,
        )
        .ok()
        .unwrap();
    let video = mp4("declared");
    let (collection, item) = smart_wearable(&node, &video).await;
    for (route, bound, error, max) in [
        (
            "videos",
            video::BODY_LIMIT,
            "The video is too large",
            250_000_000,
        ),
        (
            "files",
            crate::UPLOAD_BODY_LIMIT,
            "The upload is too large",
            crate::items::upload::MAX_BYTES as u64,
        ),
    ] {
        for declared in [bound as u64 + 1, 1_000_000_000_000_000] {
            let mut request = post(
                Some(OWNER_KEY),
                &format!("/v1/items/{item}/{route}"),
                Some("198.51.100.60"),
                Body::empty(),
            )
            .await;
            request
                .headers_mut()
                .insert(header::CONTENT_LENGTH, declared.into());
            let (status, answer, _) =
                answer(node.app.clone().oneshot(request).await.unwrap()).await;
            assert_eq!(
                (status, answer["error"].as_str(), &answer["data"]),
                (413, Some(error), &json!({ "maxBytes": max })),
                "{route} declaring {declared}"
            );
        }
    }
    cleanup(&node.pool, &[collection], &[]).await;
}

/// A client that sends part of an upload's body and leaves still pays its
/// address's day for the bytes that arrived, on both upload routes.
#[tokio::test]
async fn an_upload_its_client_leaves_still_pays_for_what_arrived() {
    let Some(node) = node_with(|_| {}, true).await else {
        return;
    };
    let video = mp4_of(4 * 1024 * 1024);
    let (collection, item) = smart_wearable(&node, &video).await;
    for (n, (route, whole)) in [
        ("videos", video_form(&video)),
        ("files", form(&[(&hash(&video), Some("blob"), &video)])),
    ]
    .into_iter()
    .enumerate()
    {
        let ip = format!("198.51.100.{}", 70 + n);
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-real-ip", ip.parse().unwrap());
        let site = crate::limits::upload_source(&headers).unwrap();
        let (body, tx, pulled) = fed();
        let request = post(
            Some(OWNER_KEY),
            &format!("/v1/items/{item}/{route}"),
            Some(&ip),
            body,
        )
        .await;
        let uploading = tokio::spawn(node.app.clone().oneshot(request));
        let sent = 3 * 1024 * 1024;
        tx.send(Bytes::copy_from_slice(&whole[..sent])).unwrap();
        pulled_to(&pulled, sent, || uploading.is_finished()).await;
        assert!(!uploading.is_finished(), "{route} answered early");
        uploading.abort();
        until("the upload ended", || {
            node.state.videos.idle() && node.state.file_uploads.in_flight() == 0
        })
        .await;
        let spent = node
            .state
            .limits
            .source_upload_mib
            .spent_at(&site, std::time::Instant::now());
        assert!(
            spent > 2.9,
            "{route}: the address paid {spent} MiB for 3 MiB"
        );
    }
    cleanup(&node.pool, &[collection], &[]).await;
}
