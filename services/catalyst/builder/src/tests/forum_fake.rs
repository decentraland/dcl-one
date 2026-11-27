//! A loopback Discourse: the calls the node makes, answered as Discourse
//! answers them, each recorded.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

/// The API key the fake takes.
pub const API_KEY: &str = "sk-example-forum";
/// The account the fake expects on reads.
pub const API_USERNAME: &str = "builder-example";
/// The category the fake files topics in.
pub const CATEGORY: &str = "12";

#[derive(Default)]
struct Topic {
    id: i64,
    slug: String,
    title: String,
    posts: Vec<i64>,
}

#[derive(Default)]
struct Inner {
    calls: Vec<Value>,
    topics: Vec<Topic>,
    posts: Vec<(i64, i64)>,
    raws: std::collections::HashMap<i64, String>,
    made_topics: i64,
    made_posts: i64,
    refuse: usize,
    lose: usize,
    hold: Option<Arc<tokio::sync::Semaphore>>,
    origin: String,
}

/// The fake, its recorded calls and the failures it is told to make.
#[derive(Clone, Default)]
pub struct FakeForum(Arc<Mutex<Inner>>);

/// Discourse's slug of `title`.
pub fn slug(title: &str) -> String {
    let mut slug = String::new();
    for c in title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "topic".into()
    } else {
        slug.into()
    }
}

fn refused(status: StatusCode, error: &str) -> Response {
    (
        status,
        Json(json!({ "action": "create_post", "errors": [error] })),
    )
        .into_response()
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "errors": ["The requested URL or resource could not be found."], "error_type": "not_found" })),
    )
        .into_response()
}

impl FakeForum {
    /// Serves the fake on loopback and answers its origin.
    pub async fn start(&self) -> String {
        let app = Router::new()
            .route("/posts.json", post(Self::create))
            .route("/posts/{file}", get(Self::read_post).put(Self::update_post))
            .with_state(self.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        self.0.lock().unwrap().origin = origin.clone();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        origin
    }

    /// The forum settings that reach this fake at `origin`.
    pub fn forum(origin: &str) -> crate::curation::forum::Forum {
        crate::curation::forum::Forum {
            url: origin.to_string(),
            api: origin.to_string(),
            api_key: API_KEY.into(),
            api_username: API_USERNAME.into(),
            category: CATEGORY.into(),
        }
    }

    /// Every call so far, each `{method, path, username, body}`.
    pub fn calls(&self) -> Vec<Value> {
        self.0.lock().unwrap().calls.clone()
    }

    /// Takes the calls made since the last take.
    pub fn take(&self) -> Vec<Value> {
        std::mem::take(&mut self.0.lock().unwrap().calls)
    }

    /// The topics made so far.
    pub fn topics(&self) -> usize {
        self.0.lock().unwrap().topics.len()
    }

    /// Refuses the next `n` posts with HTTP 500.
    pub fn refuse(&self, n: usize) {
        self.0.lock().unwrap().refuse = n;
    }

    /// Makes the next `n` posts and cuts each answer short.
    pub fn lose(&self, n: usize) {
        self.0.lock().unwrap().lose = n;
    }

    /// Holds every post until [`Self::release`].
    pub fn hold(&self) {
        self.0.lock().unwrap().hold = Some(Arc::new(tokio::sync::Semaphore::new(0)));
    }

    /// Lets held and later posts through.
    pub fn release(&self) {
        if let Some(gate) = self.0.lock().unwrap().hold.take() {
            gate.add_permits(1_000);
        }
    }

    fn record(&self, method: &str, path: &str, headers: &HeaderMap, body: Value) -> bool {
        let username = headers
            .get("api-username")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        self.0.lock().unwrap().calls.push(json!({
            "method": method, "path": path, "username": username, "body": body,
        }));
        headers.get("api-key").and_then(|v| v.to_str().ok()) == Some(API_KEY)
    }

    async fn create(State(fake): State<Self>, headers: HeaderMap, body: String) -> Response {
        let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
        if !fake.record("POST", "/posts.json", &headers, parsed.clone()) {
            return refused(
                StatusCode::FORBIDDEN,
                "You are not permitted to view the requested resource.",
            );
        }
        let gate = fake.0.lock().unwrap().hold.clone();
        if let Some(gate) = gate {
            let _ = gate.acquire().await;
        }
        let mut inner = fake.0.lock().unwrap();
        if inner.refuse > 0 {
            inner.refuse -= 1;
            return refused(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong");
        }
        let next_post = 5001 + inner.made_posts;
        let answer = if let Some(topic) = parsed["topic_id"].as_i64() {
            let Some(t) = inner.topics.iter_mut().find(|t| t.id == topic) else {
                return not_found();
            };
            t.posts.push(next_post);
            let (slug, number) = (t.slug.clone(), t.posts.len());
            inner.posts.push((next_post, topic));
            inner.made_posts += 1;
            json!({ "id": next_post, "topic_id": topic, "topic_slug": slug, "post_number": number })
        } else {
            let (Some(title), Some(raw)) = (parsed["title"].as_str(), parsed["raw"].as_str())
            else {
                return refused(StatusCode::UNPROCESSABLE_ENTITY, "Title can't be blank");
            };
            if inner
                .topics
                .iter()
                .any(|t| !t.title.is_empty() && t.title.to_lowercase() == title.to_lowercase())
            {
                return refused(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Title has already been used",
                );
            }
            let topic = 1001 + inner.made_topics;
            let slug = slug(title);
            inner.topics.push(Topic {
                id: topic,
                slug: slug.clone(),
                title: title.to_string(),
                posts: vec![next_post],
            });
            inner.posts.push((next_post, topic));
            inner.raws.insert(next_post, raw.to_string());
            inner.made_topics += 1;
            inner.made_posts += 1;
            json!({ "id": next_post, "topic_id": topic, "topic_slug": slug, "post_number": 1 })
        };
        if inner.lose > 0 {
            inner.lose -= 1;
            return Response::builder()
                .header(header::CONTENT_LENGTH, "4096")
                .body(Body::from("{"))
                .unwrap();
        }
        Json(answer).into_response()
    }

    async fn read_post(
        State(fake): State<Self>,
        headers: HeaderMap,
        Path(file): Path<String>,
    ) -> Response {
        if !fake.record("GET", &format!("/posts/{file}"), &headers, Value::Null) {
            return refused(
                StatusCode::FORBIDDEN,
                "You are not permitted to view the requested resource.",
            );
        }
        let inner = fake.0.lock().unwrap();
        let id: Option<i64> = file.strip_suffix(".json").and_then(|id| id.parse().ok());
        match inner.posts.iter().find(|(post, _)| Some(*post) == id) {
            Some((post, topic)) => Json(json!({
                "id": post, "topic_id": topic,
                "raw": inner.raws.get(post).cloned().unwrap_or_default(),
            }))
            .into_response(),
            None => not_found(),
        }
    }

    async fn update_post(
        State(fake): State<Self>,
        headers: HeaderMap,
        Path(file): Path<String>,
        body: String,
    ) -> Response {
        let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
        if !fake.record("PUT", &format!("/posts/{file}"), &headers, parsed.clone()) {
            return refused(
                StatusCode::FORBIDDEN,
                "You are not permitted to view the requested resource.",
            );
        }
        let mut inner = fake.0.lock().unwrap();
        let id: Option<i64> = file.strip_suffix(".json").and_then(|id| id.parse().ok());
        let Some((post, topic)) = inner
            .posts
            .iter()
            .find(|(post, _)| Some(*post) == id)
            .copied()
        else {
            return not_found();
        };
        let Some(raw) = parsed["raw"].as_str() else {
            return refused(StatusCode::UNPROCESSABLE_ENTITY, "Body can't be blank");
        };
        inner.raws.insert(post, raw.to_string());
        let slug = inner
            .topics
            .iter()
            .find(|t| t.id == topic)
            .map(|t| t.slug.clone())
            .unwrap_or_default();
        Json(json!({ "post": { "id": post, "topic_id": topic, "topic_slug": slug } }))
            .into_response()
    }
}
