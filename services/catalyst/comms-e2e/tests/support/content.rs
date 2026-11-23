//! A stand-in for a realm's content server and lambdas, on a loopback port of its own: it
//! describes the realm at `/about`, takes profile deployments at `POST /content/entities` the way
//! the content server reads them (a multipart form), answers what a test scripts, and serves what
//! it accepted at `GET /lambdas/profiles/{address}`.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use axum::Router;
use axum::extract::{Multipart, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use parking_lot::Mutex;
use url::Url;

/// One file part of a received form.
#[derive(Debug, Clone)]
pub struct FilePart {
    pub name: String,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

/// One deployment form as it arrived.
#[derive(Debug, Clone)]
pub struct Received {
    pub content_type: String,
    pub fields: BTreeMap<String, String>,
    pub files: Vec<FilePart>,
    pub at: Instant,
}

#[derive(Default)]
pub struct Recorded {
    /// Every form posted, in order.
    pub posts: Vec<Received>,
    /// What the next posts are answered, in order: a status and a `Retry-After`. Once it is
    /// empty, a form whose entity file hashes to its `entityId` is accepted.
    pub answers: VecDeque<(u16, Option<String>)>,
    /// The profiles served, by lowercase address.
    pub served: HashMap<String, serde_json::Value>,
    /// Profile reads at the lambdas.
    pub reads: usize,
}

#[derive(Clone)]
pub struct ContentStandIn {
    /// The address to give a stack file as its `api`: its `/about` describes the realm.
    pub api: Url,
    pub recorded: Arc<Mutex<Recorded>>,
}

impl ContentStandIn {
    pub fn posts(&self) -> usize {
        self.recorded.lock().posts.len()
    }

    pub fn answer_next(&self, status: u16, retry_after: Option<&str>) {
        self.recorded
            .lock()
            .answers
            .push_back((status, retry_after.map(str::to_owned)));
    }

    /// Serves a profile of `version` for `address`, as if it had been deployed before.
    pub fn serve(&self, address: &str, version: i64) {
        self.recorded.lock().served.insert(
            address.to_lowercase(),
            serde_json::json!({ "avatars": [{ "ethAddress": address, "version": version }] }),
        );
    }

    /// The name of the profile served for `address`.
    pub fn served_name(&self, address: &str) -> Option<String> {
        self.recorded
            .lock()
            .served
            .get(&address.to_lowercase())
            .and_then(|profile| profile.pointer("/avatars/0/name"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    }
}

#[derive(Clone)]
struct Shared {
    recorded: Arc<Mutex<Recorded>>,
    about: serde_json::Value,
}

async fn about(State(shared): State<Shared>) -> Response {
    axum::Json(shared.about.clone()).into_response()
}

async fn deploy(State(shared): State<Shared>, headers: HeaderMap, mut form: Multipart) -> Response {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let mut fields = BTreeMap::new();
    let mut files = Vec::new();
    while let Ok(Some(field)) = form.next_field().await {
        let name = field.name().unwrap_or_default().to_owned();
        let file_name = field.file_name().map(str::to_owned);
        let part_type = field.content_type().map(str::to_owned);
        let bytes = field.bytes().await.unwrap_or_default().to_vec();
        if file_name.is_some() || part_type.is_some() {
            files.push(FilePart {
                name,
                file_name,
                content_type: part_type,
                bytes,
            });
        } else {
            fields.insert(name, String::from_utf8_lossy(&bytes).into_owned());
        }
    }
    let mut recorded = shared.recorded.lock();
    recorded.posts.push(Received {
        content_type,
        fields: fields.clone(),
        files: files.clone(),
        at: Instant::now(),
    });
    if let Some((status, retry_after)) = recorded.answers.pop_front() {
        let mut response = (
            StatusCode::from_u16(status).unwrap(),
            axum::Json(serde_json::json!({ "error": "scripted" })),
        )
            .into_response();
        if let Some(value) = retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_str(&value).unwrap());
        }
        return response;
    }
    let id = fields.get("entityId").cloned().unwrap_or_default();
    let Some(entity) = files
        .iter()
        .find(|file| dcl_auth::hash_bytes_v1(&file.bytes) == id)
    else {
        return (StatusCode::BAD_REQUEST, "no file hashes to the entityId").into_response();
    };
    let entity: serde_json::Value = serde_json::from_slice(&entity.bytes).unwrap();
    let pointer = entity["pointers"][0].as_str().unwrap().to_lowercase();
    let profile = serde_json::json!({
        "avatars": entity["metadata"]["avatars"],
        "timestamp": entity["timestamp"],
    });
    recorded.served.insert(pointer, profile);
    axum::Json(serde_json::json!({ "creationTimestamp": entity["timestamp"] })).into_response()
}

async fn profile(State(shared): State<Shared>, Path(address): Path<String>) -> Response {
    let mut recorded = shared.recorded.lock();
    recorded.reads += 1;
    match recorded.served.get(&address.to_lowercase()) {
        Some(profile) => axum::Json(profile.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, "Profile not found").into_response(),
    }
}

/// Starts the stand-in. Its `/about` names `content` as the content server's public URL
/// (`None`: its own `/content`, given as a path) and its own `/lambdas/` as the lambdas'.
pub async fn start(content: Option<&str>) -> ContentStandIn {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let base = format!("http://{address}/");
    let recorded = Arc::new(Mutex::new(Recorded::default()));
    let shared = Shared {
        recorded: recorded.clone(),
        about: serde_json::json!({
            "healthy": true,
            "content": { "healthy": true, "publicUrl": content.unwrap_or("/content") },
            "lambdas": { "healthy": true, "publicUrl": format!("{base}lambdas/") },
            "comms": { "healthy": true, "adapter": "archipelago:archipelago:ws://example.invalid/ws" },
        }),
    };
    let router = Router::new()
        .route("/about", get(about))
        .route("/content/entities", post(deploy))
        .route("/lambdas/profiles/{address}", get(profile))
        .with_state(shared);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    ContentStandIn {
        api: base.parse().unwrap(),
        recorded,
    }
}
