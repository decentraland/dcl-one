//! The social family's captures (`tests/fixtures/conformance/social/`)
//! replayed against an in-process router with the live replay's semantics
//! (`tests/support/conformance/replay.rs` over `catalyst_testing::conformance`):
//! the status, the non-volatile headers the fixture records (`content-type`
//! by media type), then the body, exact bytes or JSON after both sides'
//! volatile paths.

use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use base64::Engine as _;
use catalyst_testing::conformance::diff::compare_json;
use catalyst_testing::conformance::fixture::{
    Fixture, VOLATILE_HEADERS, apply_volatile_paths, media_type,
};
use catalyst_testing::conformance::volatility::Volatility;
use serde_json::Value;
use tower::ServiceExt;

/// `tests/fixtures/conformance/social/<service>/<name>.json`.
pub fn fixture(service: &str, name: &str) -> Fixture {
    let path = PathBuf::from(env!("DCL_ONE_ROOT"))
        .join("services/catalyst/tests/fixtures/conformance/social")
        .join(service)
        .join(format!("{name}.json"));
    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap()
}

/// What `fx` differs in when its request, with its recorded headers but any
/// auth chain (nothing here re-signs), is sent to `app` under `prefix` (the
/// Catalyst mount of the captured host): `status: ...` and `header ...` notes,
/// then one entry per differing body field, `<path> missing|extra|value`
/// with `[n]` folded to `[]` (what differs, not the seeded rows' own values),
/// or `body bytes differ` for a byte body.
pub async fn replay(app: &Router, prefix: &str, name: &str, fx: &Fixture) -> Vec<String> {
    let mut uri = format!("{prefix}{}", fx.request.path);
    let query: Vec<String> = fx
        .request
        .query
        .iter()
        .flat_map(|(k, vs)| vs.iter().map(move |v| format!("{k}={v}")))
        .collect();
    if !query.is_empty() {
        uri = format!("{uri}?{}", query.join("&"));
    }
    let mut request = Request::builder()
        .method(fx.request.method.as_str())
        .uri(uri);
    for (k, v) in &fx.request.headers {
        if !k.to_ascii_lowercase().starts_with("x-identity-") {
            request = request.header(k.as_str(), v.as_str());
        }
    }
    let request = request.body(Body::empty()).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    let mut notes = Vec::new();
    if status != fx.response.status {
        notes.push(format!(
            "status: fixture={} candidate={status}",
            fx.response.status
        ));
    }
    for (key, expected) in &fx.response.headers {
        if VOLATILE_HEADERS.contains(&key.to_ascii_lowercase().as_str()) {
            continue;
        }
        // hyper writes the length of a sized body on the wire, not here
        let length = bytes.len().to_string();
        let actual = headers
            .get(key.as_str())
            .and_then(|v| v.to_str().ok())
            .or_else(|| {
                key.eq_ignore_ascii_case(header::CONTENT_LENGTH.as_str())
                    .then_some(length.as_str())
            })
            .unwrap_or("<missing>");
        let same = if key.eq_ignore_ascii_case("content-type") {
            media_type(expected) == media_type(actual)
        } else {
            expected.trim() == actual.trim()
        };
        if !same {
            notes.push(format!(
                "header {key}: fixture={expected:?} candidate={actual:?}"
            ));
        }
    }
    if let Some(b64) = &fx.response.body_bytes_b64 {
        let expected = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        if expected.as_slice() != bytes.as_ref() {
            notes.push(format!(
                "body bytes differ: fixture={:?} candidate={:?}",
                String::from_utf8_lossy(&expected),
                String::from_utf8_lossy(&bytes)
            ));
        }
        return notes;
    }
    let mut baseline = fx.response.body_json.clone().unwrap();
    let Ok(mut candidate) = serde_json::from_slice::<Value>(&bytes) else {
        notes.push(format!(
            "candidate body is not JSON: {:?}",
            String::from_utf8_lossy(&bytes)
        ));
        return notes;
    };
    apply_volatile_paths(&mut baseline, &fx.volatile_paths);
    apply_volatile_paths(&mut candidate, &fx.volatile_paths);
    let mut fields: Vec<String> = compare_json(
        name,
        "fixture",
        &baseline,
        &candidate,
        &Volatility::default(),
    )
    .into_iter()
    .map(|d| {
        let mut path = String::new();
        let mut index = false;
        for c in d.path.trim_start_matches("fixture.").chars() {
            match c {
                '[' => {
                    index = true;
                    path.push_str("[]");
                }
                ']' => index = false,
                _ if index => {}
                _ => path.push(c),
            }
        }
        let kind = if d.candidate_value == "missing" {
            "missing"
        } else if d.baseline_value == "missing" {
            "extra"
        } else {
            "value"
        };
        format!("{path} {kind}")
    })
    .collect();
    fields.sort();
    fields.dedup();
    notes.extend(fields);
    notes
}
