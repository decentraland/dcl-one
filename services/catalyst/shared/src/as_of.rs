//! The descriptive-data rule every mirrored catalogue follows (places,
//! events, communities, the worlds catalogue, the catalog
//! metadata): a read keeps serving the last copy a mirror made and says how
//! old it is with `as_of`, the time of the mirror's last complete pass, the
//! first key of a JSON object answer, and how long the copy may age before
//! a reader calls it stale with `stale_after_secs` beside it ([`stale_after`]).
//! A mirror that never completed a pass has no copy, and its reads answer
//! the unavailable envelope ([`no_copy`]). Rows no mirror feeds are the
//! node's own and live: no `as_of`.
//!
//! Money and permission data (prices, trades, the chain indexes, LAND authz,
//! credits) follow the other rule: 503 past a freshness budget.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use serde::de::IgnoredAny;
use serde_json::json;

/// The body key.
pub const FIELD: &str = "as_of";
/// The body key of the copy's stale-after budget, in seconds.
pub const STALE_AFTER_FIELD: &str = "stale_after_secs";
/// The header twins of the two keys.
pub const HEADER: &str = "x-as-of";
pub const STALE_AFTER_HEADER: &str = "x-stale-after-secs";

/// How old a copy may grow before a reader calls it stale: twice its
/// mirror's interval, so one late pass is not yet news.
pub fn stale_after(interval: Duration) -> Duration {
    interval.saturating_mul(2)
}

/// What a descriptive read answers from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    /// No mirror feeds these rows: the node's own, live.
    Live,
    /// A mirror feeds them and never completed a pass: no copy yet.
    NoCopy,
    /// The copy the mirror's last complete pass made.
    AsOf(DateTime<Utc>),
}

impl Freshness {
    /// `mirrored`: a mirror feeds these rows (configured now, or a copy
    /// already in the tables); `last_complete`: its last complete pass.
    pub fn of(mirrored: bool, last_complete: Option<NaiveDateTime>) -> Self {
        match (mirrored, last_complete) {
            (_, Some(at)) => Self::AsOf(at.and_utc()),
            (true, None) => Self::NoCopy,
            (false, None) => Self::Live,
        }
    }

    /// A read over two catalogues is as old as the older copy, and has no
    /// copy when either has none.
    pub fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::NoCopy, _) | (_, Self::NoCopy) => Self::NoCopy,
            (Self::AsOf(a), Self::AsOf(b)) => Self::AsOf(a.min(b)),
            (Self::AsOf(a), Self::Live) | (Self::Live, Self::AsOf(a)) => Self::AsOf(a),
            (Self::Live, Self::Live) => Self::Live,
        }
    }
}

/// The wire form of an `as_of` time (RFC 3339, milliseconds, `Z`).
pub fn wire(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// The unavailable envelope (the shop's and the explorer statics' shape):
/// 503 with `Retry-After`, naming the mirror and the variable that turns it on.
pub fn no_copy(source: &str, configure: &str, message: &str) -> Response {
    let body = json!({ "error": "Service Unavailable", "message": message, "source": source, "configure": configure });
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::RETRY_AFTER, "60"),
        ],
        body.to_string(),
    )
        .into_response()
}

/// `body` with `"as_of": at` and `"stale_after_secs"` as its first keys
/// when it is a JSON object whose root has no `as_of` already (a nested one
/// is the payload's own); `None` leaves it as it is (an array, an empty or
/// unparseable body).
pub fn splice(body: &[u8], at: DateTime<Utc>, stale_after: Duration) -> Option<Vec<u8>> {
    let root: HashMap<String, IgnoredAny> = serde_json::from_slice(body).ok()?;
    if root.contains_key(FIELD) {
        return None;
    }
    let start = body.iter().position(|b| *b == b'{')?;
    let mut out = Vec::with_capacity(body.len() + 64);
    out.extend_from_slice(&body[..=start]);
    out.extend_from_slice(
        format!(
            "\"{FIELD}\":\"{}\",\"{STALE_AFTER_FIELD}\":{}",
            wire(at),
            stale_after.as_secs()
        )
        .as_bytes(),
    );
    if !root.is_empty() {
        out.push(b',');
    }
    out.extend_from_slice(&body[start + 1..]);
    Some(out)
}

/// A 2xx answer with the `x-as-of` and `x-stale-after-secs` headers and,
/// when it is a JSON object, both keys spliced in; anything else as it came.
pub async fn stamp(mut response: Response, at: DateTime<Utc>, stale_after: Duration) -> Response {
    if !response.status().is_success() {
        return response;
    }
    if let Ok(v) = HeaderValue::from_str(&wire(at)) {
        response.headers_mut().insert(HEADER, v);
    }
    response
        .headers_mut()
        .insert(STALE_AFTER_HEADER, HeaderValue::from(stale_after.as_secs()));
    let json = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if !json {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    // Handlers build their JSON in memory; this only re-frames it.
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "as_of: reading the answer failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let Some(stamped) = splice(&bytes, at, stale_after) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.remove(header::ETAG);
    Response::from_parts(parts, Body::from(stamped))
}

/// A HEAD read runs as the GET it mirrors, so the stamped answer's length is
/// the GET's ([`head_of`] then drops the body); `true` when it was a HEAD.
pub fn as_get(mut request: Request) -> (Request, bool) {
    let head = request.method() == Method::HEAD;
    if head {
        *request.method_mut() = Method::GET;
    }
    (request, head)
}

/// The HEAD answer of a GET `response`: its headers with `Content-Length`
/// the body's, and no body.
pub async fn head_of(response: Response) -> Response {
    let (mut parts, body) = response.into_parts();
    if parts.headers.contains_key(header::CONTENT_LENGTH) {
        return Response::from_parts(parts, Body::empty());
    }
    match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => {
            parts
                .headers
                .insert(header::CONTENT_LENGTH, HeaderValue::from(bytes.len()));
            Response::from_parts(parts, Body::empty())
        }
        Err(e) => {
            tracing::warn!(error = %e, "as_of: reading the answer failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Whether `method` reads (the descriptive rule covers reads only; a route
/// that reads over POST lists itself where the domain matches its paths).
pub fn is_read(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD)
}

/// The middleware body each domain's own wraps: `Live` passes through,
/// `NoCopy` answers `refuse()`, `AsOf` stamps the answer with the copy's
/// time and `stale_after` ([`stale_after`] of the domain's interval).
pub async fn serve(
    freshness: Freshness,
    stale_after: Duration,
    request: Request,
    next: Next,
    refuse: impl FnOnce() -> Response,
) -> Response {
    match freshness {
        Freshness::Live => next.run(request).await,
        Freshness::NoCopy => refuse(),
        Freshness::AsOf(at) => {
            let (request, head) = as_get(request);
            let stamped = stamp(next.run(request).await, at, stale_after).await;
            if head {
                head_of(stamped).await
            } else {
                stamped
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::routing::get;
    use tower::ServiceExt;

    fn at() -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000, 250_000_000).unwrap()
    }

    #[test]
    fn freshness_reads_the_marker_and_the_switch() {
        let t = at().naive_utc();
        assert_eq!(Freshness::of(false, None), Freshness::Live);
        assert_eq!(Freshness::of(true, None), Freshness::NoCopy);
        assert_eq!(Freshness::of(true, Some(t)), Freshness::AsOf(at()));
        assert_eq!(
            Freshness::of(false, Some(t)),
            Freshness::AsOf(at()),
            "a copy left in the tables is still a copy"
        );
        let older = at() - chrono::Duration::hours(1);
        assert_eq!(
            Freshness::AsOf(at()).and(Freshness::AsOf(older)),
            Freshness::AsOf(older)
        );
        assert_eq!(
            Freshness::AsOf(at()).and(Freshness::Live),
            Freshness::AsOf(at())
        );
        assert_eq!(
            Freshness::AsOf(at()).and(Freshness::NoCopy),
            Freshness::NoCopy
        );
        assert_eq!(Freshness::Live.and(Freshness::Live), Freshness::Live);
    }

    const HOUR: Duration = Duration::from_secs(3600);

    #[test]
    fn splice_puts_as_of_first_in_an_object_only() {
        let s = |b: &str| splice(b.as_bytes(), at(), HOUR).map(|v| String::from_utf8(v).unwrap());
        assert_eq!(
            s(r#"{"ok":true,"data":[]}"#).unwrap(),
            r#"{"as_of":"2027-01-15T08:00:00.250Z","stale_after_secs":3600,"ok":true,"data":[]}"#
        );
        assert_eq!(
            s(" { } ").unwrap(),
            r#" {"as_of":"2027-01-15T08:00:00.250Z","stale_after_secs":3600 } "#
        );
        let v: serde_json::Value = serde_json::from_str(&s(r#"{"a":1}"#).unwrap()).unwrap();
        assert_eq!(v["as_of"], "2027-01-15T08:00:00.250Z");
        assert_eq!(v["stale_after_secs"], 3600);
        assert_eq!(s("[1,2]"), None);
        assert_eq!(s(""), None);
        assert_eq!(s(r#"{"as_of":"x"}"#), None, "never a second key");
        assert_eq!(s(r#"{"a":1"#), None, "not JSON: untouched");
    }

    /// Only the root's keys count: a nested `as_of` (or one inside a
    /// string) is the payload's own, and the answer is still stamped.
    #[test]
    fn splice_reads_the_root_keys_only() {
        let s = |b: &str| splice(b.as_bytes(), at(), HOUR).map(|v| String::from_utf8(v).unwrap());
        for body in [
            r#"{"data":[{"as_of":"2020-01-01T00:00:00.000Z"}]}"#,
            r#"{"note":"\"as_of\" is a key"}"#,
        ] {
            let v: serde_json::Value = serde_json::from_str(&s(body).expect(body)).unwrap();
            assert_eq!(v["as_of"], "2027-01-15T08:00:00.250Z", "{body}");
        }
    }

    /// A HEAD read answers the stamped GET's length wherever the gate sits:
    /// on a router's routes, where axum's own HEAD handling sees the stamped
    /// body, and around a router as a whole, whose HEAD answer comes back
    /// already stripped to the unstamped length.
    #[tokio::test]
    async fn a_head_read_has_the_length_of_its_stamped_get() {
        let routes =
            || Router::new().route("/obj", get(|| async { axum::Json(json!({"data": [1]})) }));
        let gate = |req: Request, next: Next| {
            serve(Freshness::AsOf(at()), HOUR, req, next, || {
                no_copy("places-mirror", "PLACES_MIRROR_UPSTREAM", "no pass yet")
            })
        };
        head_matches_get(
            "on the routes",
            routes().layer(axum::middleware::from_fn(gate)),
        )
        .await;
        head_matches_get(
            "around the router",
            tower::Layer::layer(&axum::middleware::from_fn(gate), routes()),
        )
        .await;
    }

    async fn head_matches_get<S>(wiring: &str, app: S)
    where
        S: tower::Service<Request, Response = Response, Error = std::convert::Infallible> + Clone,
    {
        let answer = |method: Method| {
            let app = app.clone();
            async move {
                let res = app
                    .oneshot(
                        Request::builder()
                            .method(method)
                            .uri("/obj")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                let length = res
                    .headers()
                    .get(header::CONTENT_LENGTH)
                    .map(|v| v.to_str().unwrap().to_string());
                let as_of = res.headers().get(HEADER).cloned();
                let body = axum::body::to_bytes(res.into_body(), usize::MAX)
                    .await
                    .unwrap();
                (length, as_of, body)
            }
        };
        let (_, get_as_of, get_body) = answer(Method::GET).await;
        let (head_length, head_as_of, head_body) = answer(Method::HEAD).await;
        assert!(
            String::from_utf8_lossy(&get_body).contains("\"as_of\""),
            "{wiring}: the GET is stamped"
        );
        assert_eq!(
            head_length,
            Some(get_body.len().to_string()),
            "{wiring}: the HEAD's length is the stamped GET's"
        );
        assert_eq!(head_as_of, get_as_of, "{wiring}");
        assert!(head_body.is_empty(), "{wiring}");
    }

    #[tokio::test]
    async fn serve_stamps_refuses_or_passes_through() {
        async fn run(freshness: Freshness, uri: &str) -> (StatusCode, String, Option<String>) {
            let app = Router::new()
                .route("/obj", get(|| async { axum::Json(json!({"data": [1]})) }))
                .route("/text", get(|| async { "plain" }))
                .layer(axum::middleware::from_fn(move |req, next| {
                    serve(freshness, HOUR, req, next, || {
                        no_copy("places-mirror", "PLACES_MIRROR_UPSTREAM", "no pass yet")
                    })
                }));
            let res = app
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let status = res.status();
            let header = res
                .headers()
                .get("x-as-of")
                .map(|v| v.to_str().unwrap().to_string());
            let body = axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap();
            (status, String::from_utf8(body.to_vec()).unwrap(), header)
        }
        let (status, body, header) = run(Freshness::AsOf(at()), "/obj").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            r#"{"as_of":"2027-01-15T08:00:00.250Z","stale_after_secs":3600,"data":[1]}"#
        );
        assert_eq!(header.as_deref(), Some("2027-01-15T08:00:00.250Z"));
        let (_, body, _) = run(Freshness::AsOf(at()), "/text").await;
        assert_eq!(body, "plain", "not JSON: untouched");
        let (status, body, _) = run(Freshness::NoCopy, "/obj").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains("places-mirror"), "{body}");
        let (status, body, header) = run(Freshness::Live, "/obj").await;
        assert_eq!(
            (status, body.as_str(), header),
            (StatusCode::OK, r#"{"data":[1]}"#, None)
        );
    }
}
