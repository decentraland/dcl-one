//! Production captures of comms-gatekeeper
//! (`tests/fixtures/conformance/social/gatekeeper/`, 2026-09-25) replayed
//! against [`crate::gatekeeper::router`] with the shared replay rules
//! (`catalyst_testing::conformance::compare`): the status, the recorded
//! non-volatile headers (`content-type` by media type) and the body, exact
//! bytes or JSON. Only the captures whose answer reads no production state
//! are replayed: refusals of an absent credential, invalid input, liveness
//! and CORS, plus `user-bans` and `scene-participants-world`, whose captured
//! answers are the empty state (no ban, no participant) the scratch rows
//! reproduce. The node runs with both bearer tokens set, as production does,
//! and an empty scratch database (`CATALYST_TEST_DATABASE_URL`; skipped
//! without it).

use std::path::PathBuf;

use catalyst_testing::conformance::compare::{
    self, compare_body, query_string, status_and_header_notes,
};
use catalyst_testing::conformance::fixture::Fixture;
use catalyst_testing::conformance::volatility::Volatility;

use super::*;

/// Every replayed capture and what the node still answers differently,
/// as `status: ...`, `header ...` and `body ...` notes. Not replayed: the
/// signed captures (their auth chains expired), the answers that read
/// production state (`scene-participants`, `-default-realm`, `status`'s
/// version and commit) and `unknown-path-404`, which `serve`'s fallback
/// answers, not this router.
const CAPTURES: &[(&str, &[&str])] = &[
    ("bans-401-unauth", &[]),
    ("cast-presenters-unauth", &[]),
    ("cast-stream-info-invalid-key", &[]),
    ("community-voice-chat-active-401-no-token", &[]),
    ("community-voice-chat-status-401-no-token", &[]),
    ("get-scene-adapter-unauth", &[]),
    ("get-server-scene-adapter-unauth", &[]),
    ("ping", &[]),
    (
        "preflight-get-scene-adapter",
        // DIVERGENCE: the node's one CORS layer (`shared::cors`, decision
        // 2026-07-31) reflects the origin and the requested headers.
        &[
            "header access-control-allow-methods: fixture=\"GET,HEAD,PUT,PATCH,POST,DELETE,OPTIONS\" candidate=\"GET,HEAD,POST,PUT,DELETE,PATCH,OPTIONS\"",
            "header access-control-allow-origin: fixture=\"*\" candidate=\"https://decentraland.org\"",
            "header access-control-max-age: fixture=\"36000\" candidate=\"86400\"",
            "header vary: fixture=\"Access-Control-Request-Headers\" candidate=\"Origin\"",
        ],
    ),
    ("private-messages-token-unauth", &[]),
    ("scene-admin-unauth", &[]),
    ("scene-bans-addresses-unauth", &[]),
    ("scene-bans-unauth", &[]),
    ("scene-participants-400-no-params", &[]),
    ("scene-participants-world", &[]),
    ("scene-stream-access-unauth", &[]),
    ("status-head", &[]),
    ("user-bans-bad-address", &[]),
    ("user-bans-unknown-address", &[]),
    ("user-bans", &[]),
    ("user-warnings-401-unauth", &[]),
    ("voice-chat-status-401-no-token", &[]),
    ("world-ban-status-no-token", &[]),
];

fn fixture(name: &str) -> Fixture {
    let path = PathBuf::from(env!("DCL_ONE_ROOT"))
        .join("services/catalyst/tests/fixtures/conformance/social/gatekeeper")
        .join(format!("{name}.json"));
    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap()
}

/// What the router answers differently from the capture `name`; its
/// request is sent with its recorded headers and body.
async fn replay(app: &axum::Router, name: &str) -> Vec<String> {
    let fx = fixture(name);
    let mut uri = fx.request.path.clone();
    if let Some(query) = query_string(&fx.request.query, None) {
        uri = format!("{uri}?{query}");
    }
    let mut request = Request::builder()
        .method(fx.request.method.as_str())
        .uri(uri);
    for (k, v) in &fx.request.headers {
        request = request.header(k.as_str(), v.as_str());
    }
    let body = match &fx.request.body {
        Some(json) => Body::from(serde_json::to_vec(json).unwrap()),
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let mut notes = status_and_header_notes(&fx, status, &|key| {
        headers
            .get(key)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
            // hyper writes the length of a sized body on the wire, not here
            .or_else(|| {
                key.eq_ignore_ascii_case("content-length")
                    .then(|| bytes.len().to_string())
            })
    });
    match compare_body(&fx, name, &bytes, &Volatility::default()) {
        compare::Body::Json(diffs) => notes.extend(diffs.into_iter().map(|d| {
            format!(
                "body {}: fixture={} candidate={}",
                d.path, d.baseline_value, d.candidate_value
            )
        })),
        compare::Body::Bytes { expected, same } => {
            if !same {
                notes.push(format!(
                    "body bytes: fixture={:?} candidate={:?}",
                    String::from_utf8_lossy(&expected),
                    String::from_utf8_lossy(&bytes)
                ));
            }
        }
        compare::Body::NotJson(_) => notes.push(format!(
            "body not JSON: {:?}",
            String::from_utf8_lossy(&bytes)
        )),
        compare::Body::BadFixture(note) => notes.push(note),
    }
    notes
}

#[tokio::test]
async fn the_gatekeeper_answers_its_stateless_captures_as_production() {
    let Some(pool) = scratch().await else { return };
    let mut settings = Settings::for_tests();
    settings.gatekeeper_auth_token = Some("example-gatekeeper-token".into());
    settings.moderator_token = Some("example-moderator-token".into());
    let app = super::super::router(state_with(pool, settings));
    let mut mismatched = Vec::new();
    for (name, expected) in CAPTURES {
        let notes = replay(&app, name).await;
        if notes != *expected {
            mismatched.push(format!("{name}: {notes:#?}"));
        }
    }
    assert!(mismatched.is_empty(), "{}", mismatched.join("\n"));
}
