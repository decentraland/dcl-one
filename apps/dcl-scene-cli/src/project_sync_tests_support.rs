use super::*;
use crate::crdt_gen::TestDir;
use axum::body::Bytes;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

pub(crate) fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// A node's builder project routes, in memory, verifying every signature
/// over the full request path.
#[derive(Default)]
pub(crate) struct Fake {
    /// id -> (owner, files_version, files, created_at)
    pub(crate) projects: HashMap<String, (String, i64, BTreeMap<String, String>, String)>,
    /// How many projects the fake made (the next one's `created_at`).
    made: usize,
    pub(crate) blobs: HashMap<String, Vec<u8>>,
    pub(crate) uploads: usize,
    pub(crate) puts: usize,
    /// Every file set a PUT sent.
    pub(crate) put_sets: Vec<BTreeMap<String, String>>,
    /// On the next file-set PUT, this lands first (another client).
    pub(crate) race: Option<(String, Vec<u8>)>,
    /// The next answer at `.0` waits: it says it arrived on `.1` and
    /// answers once `.2` fires.
    hold: Option<(Wait, oneshot::Sender<()>, oneshot::Receiver<()>)>,
}

pub(crate) type Shared = Arc<Mutex<Fake>>;

/// An answer a test holds back while it changes the folder: the file
/// set, or the download of one content.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Wait {
    Files,
    Content(String),
}

pub(crate) async fn held(fake: &Shared, at: Wait) {
    let hold = fake.lock().unwrap().hold.take_if(|h| h.0 == at);
    if let Some((_, arrived, answer)) = hold {
        arrived.send(()).unwrap();
        answer.await.unwrap();
    }
}

/// Holds the node's next answer at `at`: the receiver fires when the
/// request arrives, the sender lets the node answer.
pub(crate) fn hold(fake: &Shared, at: Wait) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
    let (arrived, on_arrival) = oneshot::channel();
    let (answer, on_answer) = oneshot::channel();
    fake.lock().unwrap().hold = Some((at, arrived, on_answer));
    (on_arrival, answer)
}

/// Runs `work` until the node has the [`hold`]ed request, then `work`
/// and `meanwhile` together to the end. A run that ends before the node
/// saw that request fails here rather than leave `meanwhile` waiting.
pub(crate) async fn while_held<T: std::fmt::Debug, M>(
    work: impl Future<Output = T>,
    arrived: oneshot::Receiver<()>,
    meanwhile: impl Future<Output = M>,
) -> (T, M) {
    let mut work = std::pin::pin!(work);
    tokio::select! {
        arrived = arrived => arrived.expect("the node dropped the held request"),
        done = &mut work => {
            panic!("the run ended before the node saw the held request: {done:?}")
        }
    }
    tokio::join!(work, meanwhile)
}

/// The request's signer; `None` answers 401 like the node.
async fn signer_of(headers: &HeaderMap, method: &str, uri: &Uri) -> Option<String> {
    crate::signed_fetch::verify_signed_fetch(headers, method, uri.path(), 60)
        .await
        .ok()
}

fn unsigned() -> Response {
    (StatusCode::UNAUTHORIZED, "unsigned").into_response()
}

fn set(version: i64, files: &BTreeMap<String, String>, created_at: &str) -> Value {
    json!({ "version": version, "files": files, "created_at": created_at })
}

async fn put_project(
    State(fake): State<Shared>,
    UrlPath(id): UrlPath<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let Some(owner) = signer_of(&headers, "put", &uri).await else {
        return unsigned();
    };
    let mut fake = fake.lock().unwrap();
    if !fake.projects.contains_key(&id) {
        fake.made += 1;
        let created_at = format!("2026-01-01T00:00:{:02}.000Z", fake.made);
        fake.projects
            .insert(id.clone(), (owner.clone(), 0, BTreeMap::new(), created_at));
    }
    let entry = &fake.projects[&id];
    if entry.0 != owner {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({ "ok": true, "data": { "id": id, "files_version": entry.1, "created_at": entry.3 } }))
        .into_response()
}

async fn get_files(
    State(fake): State<Shared>,
    UrlPath(id): UrlPath<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let Some(owner) = signer_of(&headers, "get", &uri).await else {
        return unsigned();
    };
    held(&fake, Wait::Files).await;
    let fake = fake.lock().unwrap();
    match fake.projects.get(&id) {
        None => StatusCode::NOT_FOUND.into_response(),
        Some((o, ..)) if *o != owner => StatusCode::UNAUTHORIZED.into_response(),
        Some((_, v, files, created_at)) => {
            Json(json!({ "ok": true, "data": set(*v, files, created_at) })).into_response()
        }
    }
}

async fn put_files(
    State(fake): State<Shared>,
    UrlPath(id): UrlPath<String>,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if signer_of(&headers, "put", &uri).await.is_none() {
        return unsigned();
    }
    let mut fake = fake.lock().unwrap();
    fake.puts += 1;
    fake.put_sets
        .push(serde_json::from_value(body["files"].clone()).unwrap());
    if let Some((path, bytes)) = fake.race.take() {
        let hash = hash_bytes_v1(&bytes);
        fake.blobs.insert(hash.clone(), bytes);
        let project = fake.projects.get_mut(&id).unwrap();
        project.2.insert(path, hash);
        project.1 += 1;
    }
    let blobs: HashSet<String> = fake.blobs.keys().cloned().collect();
    let project = fake.projects.get_mut(&id).unwrap();
    if body["base_version"].as_i64() != Some(project.1) {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "ok": false, "data": set(project.1, &project.2, &project.3) })),
        )
            .into_response();
    }
    let files: BTreeMap<String, String> = serde_json::from_value(body["files"].clone()).unwrap();
    let missing: BTreeSet<&String> = files.values().filter(|h| !blobs.contains(*h)).collect();
    if !missing.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "data": { "missing": missing } })),
        )
            .into_response();
    }
    project.1 += 1;
    project.2 = files;
    Json(json!({ "ok": true, "data": set(project.1, &project.2, &project.3) })).into_response()
}

async fn put_blob(
    State(fake): State<Shared>,
    UrlPath((_, hash)): UrlPath<(String, String)>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if signer_of(&headers, "put", &uri).await.is_none() {
        return unsigned();
    }
    assert_eq!(hash_bytes_v1(&body), hash);
    let mut fake = fake.lock().unwrap();
    fake.uploads += 1;
    fake.blobs.insert(hash.clone(), body.to_vec());
    Json(json!({ "ok": true, "data": { "hash": hash, "size": body.len() } })).into_response()
}

async fn get_blob(State(fake): State<Shared>, UrlPath(hash): UrlPath<String>) -> Response {
    held(&fake, Wait::Content(hash.clone())).await;
    match fake.lock().unwrap().blobs.get(&hash) {
        Some(bytes) => bytes.clone().into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub(crate) async fn fake_node() -> (String, Shared) {
    let fake: Shared = Arc::default();
    let app = Router::new()
        .route("/builder-api/v1/projects/{id}", put(put_project))
        .route(
            "/builder-api/v1/projects/{id}/files",
            get(get_files).put(put_files),
        )
        .route("/builder-api/v1/projects/{id}/blobs/{hash}", put(put_blob))
        .route("/builder-api/v1/storage/contents/{hash}", get(get_blob))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, fake)
}

pub(crate) fn opts(dir: &TestDir, node: &str) -> SyncOptions {
    SyncOptions {
        dir: dir.0.clone(),
        node: Some(node.to_string()),
        project: None,
        sign_key: None,
        prefer: None,
        dry_run: false,
    }
}

pub(crate) fn node_files(fake: &Shared, project: &str) -> BTreeMap<String, String> {
    fake.lock().unwrap().projects[project].2.clone()
}

/// Another client's edit on the node: the bytes stored, the set moved.
pub(crate) fn node_write(fake: &Shared, project: &str, path: &str, bytes: Option<&[u8]>) {
    let mut fake = fake.lock().unwrap();
    let hash = bytes.map(|b| {
        let h = hash_bytes_v1(b);
        fake.blobs.insert(h.clone(), b.to_vec());
        h
    });
    let p = fake.projects.get_mut(project).unwrap();
    match hash {
        Some(h) => p.2.insert(path.into(), h),
        None => p.2.remove(path),
    };
    p.1 += 1;
}

pub(crate) fn read(dir: &TestDir, rel: &str) -> Option<String> {
    std::fs::read_to_string(dir.0.join(rel)).ok()
}

pub(crate) fn key() -> Signer {
    Signer::Key(crate::random_test_wallet().private_key_hex())
}

pub(crate) fn kept(path: &str, why: Hold) -> Kept {
    Kept {
        path: path.into(),
        why,
    }
}
