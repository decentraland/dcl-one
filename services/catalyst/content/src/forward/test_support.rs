//! A fake upstream on the loopback for the lanes' tests: it records the
//! file probes and the deploy posts and answers the posts from a queue.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{FromRequest, Multipart, RawQuery, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::routing::{get, post};
use parking_lot::Mutex;
use serde_json::json;
use sqlx::PgPool;

use super::{Forwarder, Lane, Retry, client};
use crate::storage::Storage;

/// One deploy POST as received.
#[derive(Clone, Debug)]
pub struct Received {
    pub content_type: String,
    pub content_length: Option<u64>,
    pub body: Bytes,
}

#[derive(Default)]
struct Inner {
    held: Mutex<HashSet<String>>,
    answers: Mutex<VecDeque<(u16, String)>>,
    probed: Mutex<Vec<String>>,
    probe_status: Mutex<Option<u16>>,
    posts: Mutex<Vec<Received>>,
    posted: tokio::sync::Notify,
    stalled: Mutex<bool>,
}

pub struct FakeUpstream {
    /// `http://127.0.0.1:<port>`: the forwarder's target.
    pub base: String,
    inner: Arc<Inner>,
}

impl FakeUpstream {
    /// Serves `{prefix}/available-content` and `{prefix}/entities`
    /// (`/content` for a catalyst, `` for a worlds server).
    pub async fn start(prefix: &str) -> Self {
        let inner = Arc::new(Inner::default());
        let app = axum::Router::new()
            .route(&format!("{prefix}/available-content"), get(available))
            .route(&format!("{prefix}/entities"), post(deploy))
            .with_state(inner.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await });
        Self { base, inner }
    }

    /// `/available-content` reports `hash` as held from now on.
    pub fn hold(&self, hash: &str) {
        self.inner.held.lock().insert(hash.to_string());
    }

    /// The next deploy POST is answered `status` with `body`; once the
    /// queue is empty every POST is a 200.
    pub fn answer(&self, status: u16, body: &str) {
        self.inner
            .answers
            .lock()
            .push_back((status, body.to_string()));
    }

    pub fn posts(&self) -> Vec<Received> {
        self.inner.posts.lock().clone()
    }

    /// Returns once `n` deploy POSTs have been received.
    pub async fn posted(&self, n: usize) {
        loop {
            let next = self.inner.posted.notified();
            if self.inner.posts.lock().len() >= n {
                return;
            }
            next.await;
        }
    }

    /// Every cid asked about, in order.
    pub fn probed(&self) -> Vec<String> {
        self.inner.probed.lock().clone()
    }

    /// `/available-content` answers `status` (and no list) from now on.
    pub fn refuse_probes(&self, status: u16) {
        *self.inner.probe_status.lock() = Some(status);
    }

    /// While stalled, a deploy POST is recorded and then never answered.
    pub fn stall(&self, stalled: bool) {
        *self.inner.stalled.lock() = stalled;
    }
}

async fn available(
    State(inner): State<Arc<Inner>>,
    RawQuery(query): RawQuery,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let cids: Vec<String> = query
        .unwrap_or_default()
        .split('&')
        .filter_map(|pair| pair.strip_prefix("cid="))
        .map(|cid| urlencoding::decode(cid).unwrap().into_owned())
        .collect();
    inner.probed.lock().extend(cids.iter().cloned());
    if let Some(status) = *inner.probe_status.lock() {
        return (StatusCode::from_u16(status).unwrap(), "Invalid cid format.").into_response();
    }
    let held = inner.held.lock();
    axum::Json(json!(
        cids.iter()
            .map(|cid| json!({ "cid": cid, "available": held.contains(cid) }))
            .collect::<Vec<_>>()
    ))
    .into_response()
}

async fn deploy(
    State(inner): State<Arc<Inner>>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    let text = |name| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    inner.posts.lock().push(Received {
        content_type: text(header::CONTENT_TYPE).unwrap_or_default(),
        content_length: text(header::CONTENT_LENGTH).and_then(|v| v.parse().ok()),
        body,
    });
    inner.posted.notify_waiters();
    if *inner.stalled.lock() {
        std::future::pending::<()>().await;
    }
    let (status, body) = inner
        .answers
        .lock()
        .pop_front()
        .unwrap_or((200, r#"{"creationTimestamp":1}"#.to_string()));
    (StatusCode::from_u16(status).unwrap(), body)
}

/// A received form: the text fields by name, the file parts in order
/// (each part's file name is its field name, as catalyst-client sends).
#[derive(Debug)]
pub struct Parsed {
    pub fields: BTreeMap<String, String>,
    pub files: Vec<(String, Bytes)>,
}

/// Reads a received body with the multipart parser the deploy routes use.
pub async fn parse(received: &Received) -> Parsed {
    let request = Request::builder()
        .method("POST")
        .header(header::CONTENT_TYPE, &received.content_type)
        .body(Body::from(received.body.clone()))
        .unwrap();
    let mut form = Multipart::from_request(request, &()).await.unwrap();
    let mut parsed = Parsed {
        fields: BTreeMap::new(),
        files: Vec::new(),
    };
    while let Some(field) = form.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        match field.file_name().map(str::to_string) {
            Some(file_name) => {
                assert_eq!(file_name, name, "a file part is named by its hash");
                parsed.files.push((name, field.bytes().await.unwrap()));
            }
            None => {
                let value = field.text().await.unwrap();
                parsed.fields.insert(name, value);
            }
        }
    }
    parsed
}

/// A forwarder to a loopback `target` whose retries are an hour apart, so
/// a test moves `next_attempt_at` itself.
pub fn forwarder(
    pool: PgPool,
    storage: Arc<Storage>,
    lane: Lane,
    target: &str,
    max_attempts: i32,
) -> Arc<Forwarder> {
    Arc::new(Forwarder::new(
        pool,
        storage,
        client().unwrap().allowing_private_hosts(),
        lane,
        target,
        Retry {
            max_attempts,
            base: Duration::from_secs(3600),
        },
    ))
}
