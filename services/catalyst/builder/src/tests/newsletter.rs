//! A visitor subscribes to the newsletter through `POST /v1/newsletter`, at
//! both mounts as `serve` mounts them, and the node forwards the
//! subscription to the newsletter service as builder-server does; a
//! malformed email, a failing service, unset settings and bulk subscription
//! from one source, of one email or through the node are each answered
//! without leaking what the caller sent or what the service answered.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::response::IntoResponse;
use chrono::Duration;
use serde_json::{Value, json};

use super::validator::{advance, clock, set_clock};

const KEY: &str = "sk-example123";
const PUBLICATION: &str = "pub-example";
const SOURCE: &str = "203.0.113.9";
const OTHER_SOURCE: &str = "198.51.100.4";

#[derive(Clone, Debug, PartialEq)]
struct Received {
    method: String,
    path: String,
    content_type: Option<String>,
    accept: Option<String>,
    authorization: Option<String>,
    body: Value,
}

#[derive(Clone)]
enum Reply {
    Accept,
    Fail,
    Redirect(String),
}

#[derive(Clone)]
struct FakeNewsletter {
    received: Arc<Mutex<Vec<Received>>>,
    reply: Arc<Mutex<Reply>>,
}

impl FakeNewsletter {
    async fn start() -> (Self, String) {
        let fake = Self {
            received: Arc::default(),
            reply: Arc::new(Mutex::new(Reply::Accept)),
        };
        let recorder = fake.clone();
        let app = Router::new().fallback(move |request: Request<Body>| {
            let fake = recorder.clone();
            async move {
                let header = |headers: &HeaderMap, name: &str| {
                    headers
                        .get(name)
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string)
                };
                let (parts, body) = request.into_parts();
                let bytes = axum::body::to_bytes(body, 1 << 20).await.unwrap();
                fake.received.lock().unwrap().push(Received {
                    method: parts.method.to_string(),
                    path: parts.uri.path().to_string(),
                    content_type: header(&parts.headers, "content-type"),
                    accept: header(&parts.headers, "accept"),
                    authorization: header(&parts.headers, "authorization"),
                    body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
                });
                match fake.reply.lock().unwrap().clone() {
                    Reply::Accept => (
                        StatusCode::CREATED,
                        axum::Json(json!({"data": {"id": "sub_example"}})),
                    )
                        .into_response(),
                    Reply::Fail => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "detail-from-the-service-example",
                    )
                        .into_response(),
                    Reply::Redirect(to) => {
                        (StatusCode::FOUND, [("location", to)], "").into_response()
                    }
                }
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (fake, format!("http://{addr}/v2"))
    }

    fn received(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }

    fn reply(&self, reply: Reply) {
        *self.reply.lock().unwrap() = reply;
    }
}

struct Node {
    app: Router,
    pool: sqlx::PgPool,
}

async fn node_with(url: Option<String>) -> Option<Node> {
    node_tuned(url, |_| ()).await
}

async fn node_tuned(url: Option<String>, tune: impl FnOnce(&mut crate::State)) -> Option<Node> {
    set_clock(
        chrono::DateTime::from_timestamp_millis(chrono::Utc::now().timestamp_millis()).unwrap(),
    );
    let pool = crate::db::scratch::isolated_pool(2).await?;
    let super::support::Node { pool, app, .. } = super::support::node_over(
        pool,
        super::support::temp_storage().await,
        None,
        crate::Settings {
            writes_enabled: true,
            newsletter: url.map(|url| crate::NewsletterForward {
                url,
                publication_id: PUBLICATION.into(),
                api_key: KEY.into(),
            }),
            ..crate::Settings::default()
        },
        |state| {
            state.items.clock.now = clock;
            tune(state);
        },
    )
    .await;
    Some(Node { app, pool })
}

async fn subscribe(
    node: &Node,
    path: &str,
    source: Option<&str>,
    body: &str,
) -> (u16, String, Option<String>) {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(source) = source {
        request = request.header("x-real-ip", source);
    }
    let response = tower::ServiceExt::oneshot(
        node.app.clone(),
        request.body(Body::from(body.to_string())).unwrap(),
    )
    .await
    .unwrap();
    let status = response.status().as_u16();
    let retry = response
        .headers()
        .get("retry-after")
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned(), retry)
}

fn email(n: usize) -> String {
    format!("jane.doe+{n}@example.com")
}

fn body_for(email: &str) -> String {
    json!({ "email": email }).to_string()
}

const OK: &str = r#"{"ok":true}"#;
const TOO_MANY: &str =
    r#"{"data":{},"error":"Too many requests, try again in a minute","ok":false}"#;

#[tokio::test]
async fn a_visitor_subscribes_and_the_service_receives_builder_servers_request() {
    let (fake, url) = FakeNewsletter::start().await;
    let Some(node) = node_with(Some(url)).await else {
        return;
    };

    let answer = subscribe(
        &node,
        "/v1/newsletter",
        Some(SOURCE),
        r#"{"email":" Jane.Doe@Example.com ","source":"auth"}"#,
    )
    .await;
    assert_eq!((answer.0, answer.1.as_str()), (200, OK));
    let answer = subscribe(
        &node,
        "/builder-api/v1/newsletter",
        None,
        &body_for("john.doe@example.com"),
    )
    .await;
    assert_eq!((answer.0, answer.1.as_str()), (200, OK));

    let expected = |email: &str, source: &str| Received {
        method: "POST".into(),
        path: format!("/v2/publications/{PUBLICATION}/subscriptions"),
        content_type: Some("application/json".into()),
        accept: Some("application/json".into()),
        authorization: Some(format!("Bearer {KEY}")),
        body: json!({
            "email": email,
            "reactivate_existing": true,
            "send_welcome_email": false,
            "utm_source": source,
            "utm_medium": "organic",
        }),
    };
    assert_eq!(
        fake.received(),
        vec![
            expected("jane.doe@example.com", "auth"),
            expected("john.doe@example.com", "Builder"),
        ]
    );
    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM builder.newsletter_subscriptions \
         WHERE email IN ('jane.doe@example.com', 'john.doe@example.com')",
    )
    .fetch_one(&node.pool)
    .await
    .unwrap();
    assert_eq!(stored, 0, "the node keeps no subscriber's email");
}

#[tokio::test]
async fn a_malformed_or_oversized_subscription_is_refused_without_echoing_it() {
    let (fake, url) = FakeNewsletter::start().await;
    let Some(node) = node_with(Some(url)).await else {
        return;
    };

    let long = format!("{}@example.com", "a".repeat(250));
    let injected = "jane.doe@example.com\r\nbcc: victim@example.com";
    for body in [
        "{}".to_string(),
        "not json".to_string(),
        r#"{"email":42}"#.to_string(),
        body_for("not-an-email"),
        body_for(&long),
        body_for(injected),
        json!({ "email": "jane.doe@example.com", "source": "s".repeat(101) }).to_string(),
        json!({ "email": "jane.doe@example.com", "source": "line\nbreak" }).to_string(),
    ] {
        let (status, answer, _) = subscribe(&node, "/v1/newsletter", Some(SOURCE), &body).await;
        assert_eq!(status, 400, "{body}");
        assert!(
            !answer.contains("example.com")
                && !answer.contains("aaaa")
                && !answer.contains("break"),
            "{answer}"
        );
    }
    let oversized =
        json!({ "email": "jane.doe@example.com", "padding": "p".repeat(8 * 1024) }).to_string();
    let (status, answer, _) =
        subscribe(&node, "/v1/newsletter", Some(OTHER_SOURCE), &oversized).await;
    assert_eq!(status, 413, "{answer}");
    assert!(!answer.contains("example.com"), "{answer}");

    assert_eq!(fake.received(), vec![]);
}

#[tokio::test]
async fn a_failing_newsletter_service_answers_502_and_keeps_its_answer_to_itself() {
    let (fake, url) = FakeNewsletter::start().await;
    let (elsewhere, elsewhere_url) = FakeNewsletter::start().await;
    let Some(node) = node_with(Some(url)).await else {
        return;
    };
    let refused =
        r#"{"data":{},"error":"The newsletter service did not take the subscription","ok":false}"#;

    fake.reply(Reply::Fail);
    let (status, answer, _) =
        subscribe(&node, "/v1/newsletter", Some(SOURCE), &body_for(&email(1))).await;
    assert_eq!((status, answer.as_str()), (502, refused));

    fake.reply(Reply::Redirect(format!(
        "{elsewhere_url}/publications/{PUBLICATION}/subscriptions"
    )));
    let (status, answer, _) =
        subscribe(&node, "/v1/newsletter", Some(SOURCE), &body_for(&email(2))).await;
    assert_eq!((status, answer.as_str()), (502, refused));
    assert_eq!(fake.received().len(), 2);
    assert_eq!(
        elsewhere.received(),
        vec![],
        "the key never follows a redirect"
    );

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let closed = format!("http://{}/v2", listener.local_addr().unwrap());
    drop(listener);
    let Some(down) = node_with(Some(closed)).await else {
        return;
    };
    let (status, answer, _) =
        subscribe(&down, "/v1/newsletter", Some(SOURCE), &body_for(&email(3))).await;
    assert_eq!((status, answer.as_str()), (502, refused));
}

#[tokio::test]
async fn without_its_settings_the_route_answers_as_builder_server_does() {
    let Some(node) = node_with(None).await else {
        return;
    };
    let answer = subscribe(&node, "/v1/newsletter", Some(SOURCE), &body_for(&email(1))).await;
    assert_eq!((answer.0, answer.1.as_str()), (200, OK));
}

#[tokio::test]
async fn bulk_subscription_from_one_source_is_cut_off_until_its_window_passes() {
    let (fake, url) = FakeNewsletter::start().await;
    let Some(node) = node_with(Some(url)).await else {
        return;
    };

    for n in 0..crate::limits::NEWSLETTER_PER_SOURCE as usize {
        let answer = subscribe(&node, "/v1/newsletter", Some(SOURCE), &body_for(&email(n))).await;
        assert_eq!(answer.0, 200, "{n}: {}", answer.1);
    }
    let (status, answer, retry) = subscribe(
        &node,
        "/v1/newsletter",
        Some(SOURCE),
        &body_for(&email(100)),
    )
    .await;
    assert_eq!(
        (status, answer.as_str(), retry.as_deref()),
        (429, TOO_MANY, Some("360"))
    );
    let (status, _, _) = subscribe(
        &node,
        "/builder-api/v1/newsletter",
        Some(SOURCE),
        "not json",
    )
    .await;
    assert_eq!(
        status, 429,
        "the source's budget is checked before its body"
    );
    assert_eq!(
        fake.received().len(),
        crate::limits::NEWSLETTER_PER_SOURCE as usize
    );

    let answer = subscribe(
        &node,
        "/v1/newsletter",
        Some(OTHER_SOURCE),
        &body_for(&email(101)),
    )
    .await;
    assert_eq!(
        answer.0, 200,
        "another source keeps its own budget: {}",
        answer.1
    );

    advance(Duration::seconds(359));
    assert_eq!(
        subscribe(
            &node,
            "/v1/newsletter",
            Some(SOURCE),
            &body_for(&email(102))
        )
        .await
        .0,
        429
    );
    advance(Duration::seconds(1));
    assert_eq!(
        subscribe(
            &node,
            "/v1/newsletter",
            Some(SOURCE),
            &body_for(&email(102))
        )
        .await
        .0,
        200
    );
    assert_eq!(
        subscribe(
            &node,
            "/v1/newsletter",
            Some(SOURCE),
            &body_for(&email(103))
        )
        .await
        .0,
        429
    );
}

#[tokio::test]
async fn one_email_is_subscribed_at_most_three_times_a_day_whatever_its_source() {
    let (fake, url) = FakeNewsletter::start().await;
    let Some(node) = node_with(Some(url)).await else {
        return;
    };

    for n in 0..crate::limits::NEWSLETTER_PER_EMAIL {
        let source = format!("203.0.113.{}", 20 + n);
        let answer = subscribe(
            &node,
            "/v1/newsletter",
            Some(&source),
            &body_for("Jane.Doe@example.com"),
        )
        .await;
        assert_eq!(answer.0, 200, "{}", answer.1);
    }
    let (status, answer, retry) = subscribe(
        &node,
        "/v1/newsletter",
        Some("203.0.113.50"),
        &body_for("jane.doe@EXAMPLE.com"),
    )
    .await;
    assert_eq!(
        (status, answer.as_str(), retry.as_deref()),
        (429, TOO_MANY, Some("28800"))
    );
    let answer = subscribe(
        &node,
        "/v1/newsletter",
        Some("203.0.113.50"),
        &body_for("john.doe@example.com"),
    )
    .await;
    assert_eq!(answer.0, 200, "{}", answer.1);
    assert_eq!(fake.received().len(), 4);
}

#[tokio::test]
async fn the_node_forwards_at_most_its_own_bound_whatever_the_sources() {
    let (fake, url) = FakeNewsletter::start().await;
    let Some(node) = node_tuned(Some(url), |state| {
        state.limits.newsletter_node =
            crate::limits::budget(2, std::time::Duration::from_secs(3_600));
    })
    .await
    else {
        return;
    };

    for n in 0..2 {
        let source = format!("203.0.113.{}", 60 + n);
        assert_eq!(
            subscribe(&node, "/v1/newsletter", Some(&source), &body_for(&email(n)))
                .await
                .0,
            200
        );
    }
    let (status, _, retry) = subscribe(
        &node,
        "/v1/newsletter",
        Some("203.0.113.70"),
        &body_for(&email(9)),
    )
    .await;
    assert_eq!((status, retry.as_deref()), (429, Some("1800")));
    assert_eq!(fake.received().len(), 2);
}
