// from: catalyrst/crates/catalyrst-conformance/fixtures/notifications/*.json (the fixtures under tests/fixtures/notifications/), catalyrst/crates/catalyrst-conformance/src/bin/replay.rs (the comparison rules)
//! The notifications conformance fixtures replayed against the in-process
//! router as `serve` mounts it (nested under `/notifications`). The old
//! fixtures are answer bodies only, so each is paired here with the request
//! that produces it and compared by shape: the key set of every object (the
//! first element of an array), scalars loosely, plus the values that are not
//! volatile (flags, counts, labels). Needs CATALYST_TEST_DATABASE_URL; one
//! scratch database for the file.

use std::sync::Arc;

use alloy::signers::{Signer, local::PrivateKeySigner};
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use catalyst::auth::{AuthLink, AuthLinkType};
use catalyst::http::Outbound;
use catalyst::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};
use catalyst::social::notifications::{Settings, State, router};

#[path = "support/db.rs"]
mod db;

const ADMIN: &str = "fixture-admin-token";
const METADATA: &str = r#"{"signer":"dcl:explorer"}"#;

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/notifications/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")))
        .unwrap()
}

/// The walker's shape: object -> its keys with their shapes, array -> the
/// first element's shape (or "empty"), scalar -> "scalar" (null included).
fn shape(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), shape(v))).collect()),
        Value::Array(a) => json!({ "array": a.first().map(shape).unwrap_or(json!("empty")) }),
        _ => json!("scalar"),
    }
}

fn assert_shape(name: &str, answer: &Value) {
    let expected = fixture(name);
    assert_eq!(
        shape(answer),
        shape(&expected),
        "{name}: answer {answer} does not have the fixture's shape {expected}"
    );
}

struct Wallet(PrivateKeySigner);

impl Wallet {
    fn address(&self) -> String {
        format!("{:#x}", self.0.address()).to_lowercase()
    }

    async fn headers(&self, method: &str, path: &str) -> Vec<(String, String)> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let payload = build_payload_v6(method, path, &timestamp, METADATA);
        let signature = self
            .0
            .sign_message(payload.as_bytes())
            .await
            .unwrap()
            .to_string();
        let chain = [
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: format!("{:#x}", self.0.address()),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload,
                signature: Some(signature),
            },
        ];
        let mut out = vec![
            (AUTH_TIMESTAMP_HEADER.to_string(), timestamp),
            (AUTH_METADATA_HEADER.to_string(), METADATA.to_string()),
        ];
        for (i, link) in chain.iter().enumerate() {
            out.push((
                format!("{AUTH_CHAIN_HEADER_PREFIX}{i}"),
                serde_json::to_string(link).unwrap(),
            ));
        }
        out
    }
}

async fn send(
    app: &Router,
    method: &str,
    path: &str,
    headers: Vec<(String, String)>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in &headers {
        req = req.header(k, v);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()))
    };
    (status, value)
}

#[tokio::test]
async fn the_notification_fixtures_replay_against_the_mounted_router() {
    let Some(scratch) = db::scratch_database().await else {
        return;
    };
    let state = Arc::new(State::new(
        scratch.pool.clone(),
        Settings {
            admin_token: Some(ADMIN.to_string()),
            ..Settings::default()
        },
        Outbound::for_tests(),
        None,
        None,
    ));
    let app = Router::new().nest("/notifications", router(state));
    let me = Wallet(PrivateKeySigner::random());
    let other = Wallet(PrivateKeySigner::random());

    // auth-error-production.json: an unsigned request answers upstream's envelope byte
    // for byte (no `ok`, the ADR-44 sentence as the message).
    let (status, body) = send(&app, "GET", "/notifications/notifications", vec![], None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body, fixture("auth-error-production"));

    // broadcast.json: the admin bearer broadcasts one badge_granted to two wallets.
    let broadcast = json!({ "type": "badge_granted", "metadata": { "badgeId": "welcome" }, "addresses": [me.address(), other.address()] });
    let (status, body) = send(
        &app,
        "POST",
        "/notifications/notifications/broadcast",
        vec![("authorization".into(), format!("Bearer {ADMIN}"))],
        Some(broadcast.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_shape("broadcast", &body);
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["type"], json!("badge_granted"));
    assert_eq!(body["recipients"], json!(2));
    assert!(Uuid::parse_str(body["broadcastId"].as_str().unwrap()).is_ok());
    let (status, body) = send(
        &app,
        "POST",
        "/notifications/notifications/broadcast",
        vec![],
        Some(broadcast),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no bearer: {body}");

    // list-response.json: the recipient lists it (signed fetch), one item with the fixture's keys.
    let (status, body) = send(
        &app,
        "GET",
        "/notifications/notifications?limit=10",
        me.headers("get", "/notifications/notifications").await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_shape("list-response", &body);
    let item = &body["notifications"][0];
    assert_eq!(item["type"], json!("badge_granted"));
    assert_eq!(item["address"], json!(me.address()));
    assert_eq!(item["read"], json!(false));
    assert_eq!(item["metadata"]["badgeId"], json!("welcome"));
    assert!(
        item["timestamp"].as_str().unwrap().parse::<i64>().is_ok(),
        "epoch ms as a string: {item}"
    );
    let id = item["id"].as_str().unwrap().to_string();

    // mark-read.json: one of the fixture's three ids exists here, so `updated` is 1.
    let (status, body) = send(
        &app,
        "PUT",
        "/notifications/notifications/read",
        me.headers("put", "/notifications/notifications/read").await,
        Some(json!({ "notificationIds": [id, Uuid::new_v4().to_string(), Uuid::new_v4().to_string()] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_shape("mark-read", &body);
    assert_eq!(body["updated"], json!(1));

    // subscription.json: a pending email makes `unconfirmedEmail` appear (set-email writes the
    // row, then answers 500 because no mailer is configured, as the old service did).
    let (status, _) = send(
        &app,
        "PUT",
        "/notifications/set-email",
        me.headers("put", "/notifications/set-email").await,
        Some(json!({ "email": "pending@example.com", "isCreditsWorkflow": false })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "the unconfigured mailer answers 500 after writing the pending row"
    );
    let (status, body) = send(
        &app,
        "GET",
        "/notifications/subscription",
        me.headers("get", "/notifications/subscription").await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The fixture's `details` lists the two flags only; the service answers the full
    // document (the flags plus one `{email, in_app}` pair per NOTIFICATION_TYPES entry),
    // so the fixture is the floor of the key set here, not the whole of it.
    let expected = fixture("subscription");
    for (key, value) in expected.as_object().unwrap() {
        if key == "details" {
            for (flag, v) in value.as_object().unwrap() {
                assert_eq!(shape(&body["details"][flag]), shape(v), "details.{flag}");
            }
        } else {
            assert_eq!(shape(&body[key]), shape(value), "{key}");
        }
    }
    assert_eq!(
        body.as_object().unwrap().len(),
        expected.as_object().unwrap().len(),
        "{body}"
    );
    assert!(
        body["details"]["message_type"]["badge_granted"]["in_app"].is_boolean(),
        "{body}"
    );
    assert_eq!(body["address"], json!(me.address()));
    assert_eq!(body["unconfirmedEmail"], json!("pending@example.com"));
    assert_eq!(body["details"]["ignore_all_email"], json!(false));
    assert_eq!(body["details"]["ignore_all_in_app"], json!(false));

    // opt-out-created.json and community-opt-out.json.
    let community = Uuid::new_v4().to_string();
    let (status, body) = send(
        &app,
        "POST",
        "/notifications/subscription/opt-outs",
        me.headers("post", "/notifications/subscription/opt-outs")
            .await,
        Some(json!({ "scope": "community", "scopeId": community })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_shape("opt-out-created", &body);
    assert_eq!(body, json!({ "ok": true }));
    let path = format!("/notifications/subscription/opt-outs/community/{community}");
    let (status, body) = send(&app, "GET", &path, me.headers("get", &path).await, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_shape("community-opt-out", &body);
    assert_eq!(
        body,
        json!({ "scope": "community", "scopeId": community, "optedOut": true })
    );

    scratch.teardown().await;
}
