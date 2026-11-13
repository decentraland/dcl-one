//! Registry gateway without a response cache. Profiles are served by Catalyst
//! itself (`shop::market::profiles::registry`); the read routes below go to the
//! backend named by `EXPLORER_REGISTRY_SOURCE_URL`:
//!
//! - a loopback URL is the local abgen runtime with native Registry state; it
//!   also receives the signed-fetch headers and the bound `x-original-path`;
//! - any other URL is a public Registry upstream behind the usual source SSRF
//!   guard; it receives only the request itself;
//! - empty answers the boot reads (`/entities/active`, `/entities/versions`)
//!   with `200 []` and the other reads with 503.
//!
//! The boot reads check the Registry's request rule first, with every source:
//! a body without a non-empty `pointers` answers the Registry's
//! `400 {"ok":false,"message":"No pointers provided"}` (`ids` is not read),
//! and a body that is not JSON its `500`. An `/entities/active` answer drops
//! every entity whose id is on this node's content denylist, as the content
//! reads do ([`crate::content::queries::retain_non_denylisted`]).
//!
//! Registry writes (moderation, queue controls, registration, cache flush)
//! are not exposed here: the abgen admin bearer stays on this host and those
//! operations go through the ops console. No `Authorization` header is forwarded.
//!
//! Both halves of `/asset-bundle-registry` answer CORS as the Registry's own
//! http-server does ([`cors`]). The read half also answers the
//! well-known-components status checks (`/health/live`, `/health/ready`,
//! `/health/startup`) and holds the prefix's fallback, so a path neither half
//! routes still gets the Registry's CORS and its `404 Not found`.

use std::sync::LazyLock;
use std::time::Duration;

use axum::extract::{OriginalUri, Request, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes;
use serde_json::{Value, json};
use tokio::sync::Semaphore;

use super::{Settings, Shared};

/// Largest backend response buffered for one request. Registry answers are
/// far smaller; this bounds the worst case at `IN_FLIGHT * BODY_CAP` (512 MiB).
const BODY_CAP: usize = 8 << 20;
/// Backend requests in flight at once, so the buffered responses stay bounded
/// at `IN_FLIGHT * BODY_CAP`. Nothing is cached: every read is a backend call.
const IN_FLIGHT: usize = 64;
/// How long a read waits for a free slot before it is refused with 503: a
/// boot burst queues briefly instead of failing at once.
const SLOT_WAIT: Duration = Duration::from_secs(2);
const TIMEOUT: Duration = Duration::from_secs(30);
static PERMITS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(IN_FLIGHT));

pub fn routes(state: Shared) -> Router {
    Router::new()
        .route("/entities/active", post(active))
        .route("/entities/versions", post(versions))
        .route("/entities/status", get(read))
        .route("/entities/status/{id}", get(read))
        .route("/worlds/{world_name}/manifest", get(read))
        .route("/denylist", get(read))
        .route("/queues/status", get(read))
        .route("/status", get(status))
        .route("/health", get(health))
        .with_state(state)
        // The Registry names its http server `server` (`components.ts`).
        .merge(crate::shared::health::status_checks_router_with(
            crate::shared::health::SERVER_PASS,
        ))
        .fallback(not_found)
        .layer(axum::middleware::from_fn(cors))
}

/// `@dcl/http-server`'s answer for a path no route serves
/// (`captures/abr/AB-12-unknown-route.json`).
async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

/// The Registry's CORS: `@dcl/http-server` with the methods its
/// `createServerComponent` lists and `maxAge: 86400` (`components.ts`),
/// captured as `captures/abr/AB-12-preflight.json`
/// ([`crate::shared::cors::HttpServerCors`]).
const CORS: crate::shared::cors::HttpServerCors = crate::shared::cors::HttpServerCors {
    methods: Some("GET,HEAD,OPTIONS,DELETE,POST,PUT"),
    max_age: "86400",
    nosniff: false,
    preflight_echoes_request: false,
};

/// The Registry's CORS ([`CORS`]): every `OPTIONS` is a 204 preflight with
/// origin `*`, with or without `Origin`; any other response gets origin `*`
/// only when the request carries an `Origin`, and never exposed headers.
pub async fn cors(req: Request, next: Next) -> Response {
    CORS.run(req, next).await
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Backend {
    /// The local abgen runtime (loopback, the same rule as `ABGEN_ADMIN_URL`).
    Native(url::Url),
    /// A public Registry; the caller still applies the source SSRF guard.
    Upstream(url::Url),
}

impl Backend {
    /// The structural rules every Registry source meets: `http(s)`, a host,
    /// no credentials, query or fragment. A loopback host is native.
    pub(crate) fn parse(base: &str) -> anyhow::Result<Self> {
        let url = url::Url::parse(base)?;
        anyhow::ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "a Registry source is an http(s) base URL without credentials, query or fragment"
        );
        let loopback = match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
            None => false,
        };
        Ok(if loopback {
            Self::Native(url)
        } else {
            Self::Upstream(url)
        })
    }
}

/// `POST /entities/active`: the backend's entities without those on this
/// node's content denylist.
async fn active(
    State(state): State<Shared>,
    OriginalUri(original_uri): OriginalUri,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(response) = pointers_rule(&body) {
        return response;
    }
    if state.settings.sources.registry.is_empty() {
        return Json(json!([])).into_response();
    }
    match forward(&state, &original_uri, method, &uri, &headers, body).await {
        Ok((status, out_headers, bytes)) if status == StatusCode::OK => {
            let bytes = without_denylisted(bytes, &state.content.denylist);
            (status, out_headers, bytes).into_response()
        }
        Ok(answer) => answer.into_response(),
        Err(response) => response,
    }
}

/// `POST /entities/versions`: the backend's answer as received.
async fn versions(
    state: State<Shared>,
    original_uri: OriginalUri,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(response) = pointers_rule(&body) {
        return response;
    }
    if state.settings.sources.registry.is_empty() {
        return Json(json!([])).into_response();
    }
    read(state, original_uri, method, uri, headers, body).await
}

/// The Registry's request rule for the boot reads (`get-active-entities.ts`
/// and `get-entity-versions.ts`): the body is read as JSON whatever its
/// `Content-Type` (unparseable: the http-commons `500`), and `pointers` must
/// be present, truthy and, when an array, non-empty. `ids` is not read, and
/// the number of pointers is not capped.
fn pointers_rule(body: &[u8]) -> Result<(), Response> {
    let Ok(body) = serde_json::from_slice::<Value>(body) else {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"Internal Server Error"})),
        )
            .into_response());
    };
    let present = match body.get("pointers") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|n| n != 0.0),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(_) => true,
    };
    if present {
        Ok(())
    } else {
        // Written out: `json!` would sort the keys, the Registry sends `ok` first.
        Err((
            StatusCode::BAD_REQUEST,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            r#"{"ok":false,"message":"No pointers provided"}"#,
        )
            .into_response())
    }
}

/// The entities of a backend answer without those whose id is on this node's
/// content denylist (the projection and rule of the content reads, failing
/// closed on an entity without an id). An answer that drops nothing, or that
/// is not a JSON array, keeps its bytes.
fn without_denylisted(body: Vec<u8>, denylist: &crate::content::Denylist) -> Vec<u8> {
    use crate::content::queries::{EntityDoc, retain_non_denylisted};
    #[derive(serde::Deserialize)]
    struct Id<'a> {
        #[serde(borrow, default)]
        id: Option<std::borrow::Cow<'a, str>>,
    }
    let Ok(items) = serde_json::from_slice::<Vec<&serde_json::value::RawValue>>(&body) else {
        return body;
    };
    let mut docs: Vec<EntityDoc> = items
        .iter()
        .map(|raw| EntityDoc {
            id: serde_json::from_str::<Id>(raw.get())
                .ok()
                .and_then(|entity| entity.id)
                .map(|id| id.into_owned())
                .unwrap_or_default(),
            json: Bytes::copy_from_slice(raw.get().as_bytes()),
        })
        .collect();
    let before = docs.len();
    retain_non_denylisted(&mut docs, denylist);
    if docs.len() == before {
        return body;
    }
    EntityDoc::json_array(&docs).to_vec()
}

async fn read(
    State(state): State<Shared>,
    OriginalUri(original_uri): OriginalUri,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match forward(&state, &original_uri, method, &uri, &headers, body).await {
        Ok(answer) => answer.into_response(),
        Err(response) => response,
    }
}

/// One backend call: its status, the kept headers and the buffered body, or
/// the gateway's own answer when the call cannot be made or fails.
async fn forward(
    state: &Shared,
    original_uri: &Uri,
    method: Method,
    uri: &Uri,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, HeaderMap, Vec<u8>), Response> {
    let source = &state.settings.sources.registry;
    if source.is_empty() {
        return Err(message(
            StatusCode::SERVICE_UNAVAILABLE,
            "Registry service is not configured",
        ));
    }
    let (native, mut url) = match backend(&state.settings, source) {
        Ok(backend) => backend,
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "Registry source refused");
            return Err(message(
                StatusCode::SERVICE_UNAVAILABLE,
                "Registry service is not configured",
            ));
        }
    };
    let path = format!("{}{}", url.path().trim_end_matches('/'), uri.path());
    url.set_path(&path);
    if url.path() != path {
        // Encoded dot segments the URL parser would normalize out of the route.
        return Err(message(StatusCode::BAD_REQUEST, "Invalid Registry path"));
    }
    url.set_query(uri.query());
    let Some(_permit) = slot(&PERMITS, SLOT_WAIT).await else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            [("retry-after", "1")],
            Json(json!({"ok":false,"message":"Registry service is busy"})),
        )
            .into_response());
    };
    let result = async {
        let mut http = state.http.clone();
        if native {
            http = http.allowing_configured_provider_hosts();
        }
        let mut request = http
            .request(method, url)
            .max_redirects(0)
            .timeout(TIMEOUT)
            .body(body);
        for (name, value) in headers {
            let name = name.as_str();
            if matches!(
                name,
                "content-type" | "accept" | "if-none-match" | "if-modified-since"
            ) || (native && name.starts_with("x-identity-"))
            {
                request = request.header(name, value);
            }
        }
        if native {
            // Never trust a caller-supplied path: the native verifier binds
            // signed fetch to the path Catalyst actually received, which may
            // carry the gateway's Registry prefix. The query is not signed.
            request = request.header("x-original-path", original_uri.path());
        }
        let mut response = request.send().await?;
        let status = response.status();
        let mut out_headers = HeaderMap::new();
        for name in [
            "content-type",
            "allow",
            "cache-control",
            "retry-after",
            "etag",
            "last-modified",
        ] {
            if let Some(value) = response.headers().get(name) {
                out_headers.insert(name, value.clone());
            }
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            anyhow::ensure!(
                bytes.len().saturating_add(chunk.len()) <= BODY_CAP,
                "Registry response exceeds body limit"
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok::<_, anyhow::Error>((status, out_headers, bytes))
    }
    .await;
    result.map_err(|error| {
        tracing::warn!(error = %format!("{error:#}"), "Registry request failed");
        message(StatusCode::BAD_GATEWAY, "Registry service unavailable")
    })
}

/// A backend slot, waiting at most `wait` for one to free up.
pub(crate) async fn slot(
    permits: &Semaphore,
    wait: Duration,
) -> Option<tokio::sync::SemaphorePermit<'_>> {
    tokio::time::timeout(wait, permits.acquire())
        .await
        .ok()?
        .ok()
}

/// `(native, base URL)`; an upstream goes through the SSRF guard on every call.
fn backend(settings: &Settings, source: &str) -> anyhow::Result<(bool, url::Url)> {
    Ok(match Backend::parse(source)? {
        Backend::Native(url) => (true, url),
        Backend::Upstream(url) => {
            settings.guarded_url(source)?;
            (false, url)
        }
    })
}

fn message(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"ok":false,"message":message}))).into_response()
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn status(State(state): State<Shared>) -> Json<Value> {
    Json(json!({"data": {
        "version": state.settings.version,
        "currentTime": chrono::Utc::now().timestamp_millis(),
        "commitHash": state.settings.commit_hash,
    }}))
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use serde_json::Value;
    use tower::ServiceExt;

    async fn app() -> axum::Router {
        super::super::router(super::super::tests_support::state_without_db().await)
    }

    async fn send(
        app: &axum::Router,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
    ) -> axum::response::Response {
        let mut request = Request::builder().method(method).uri(path);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        app.clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    /// A preflight answers exactly the captured Registry headers; only the
    /// Registry leaves the gateway's permissive CORS.
    #[tokio::test]
    async fn registry_cors_matches_the_captured_preflight() {
        let capture: Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/content-lambdas/captures/abr/AB-12-preflight.json"
        )))
        .unwrap();
        let request_headers: Vec<(&str, &str)> = capture["request"]["headers"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str().unwrap()))
            .collect();
        let app = app().await;
        for path in [
            "/asset-bundle-registry/entities/active",
            "/asset-bundle-registry/entities/status",
            "/asset-bundle-registry/health/live",
        ] {
            let response = send(&app, "OPTIONS", path, &request_headers).await;
            assert_eq!(
                response.status().as_u16() as u64,
                capture["response"]["status"].as_u64().unwrap(),
                "{path}"
            );
            let headers: serde_json::Map<String, Value> = response
                .headers()
                .iter()
                // The capture keeps the CORS headers; axum also adds framing
                // (`content-length`) and `Allow` for a route without an
                // OPTIONS handler.
                .filter(|(name, _)| name.as_str().starts_with("access-control-") || *name == "vary")
                .map(|(name, value)| (name.to_string(), Value::from(value.to_str().unwrap())))
                .collect();
            assert_eq!(
                Value::Object(headers),
                capture["response"]["headers"],
                "{path}"
            );
            assert!(
                to_bytes(response.into_body(), 1024)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }

        let plain = send(&app, "GET", "/asset-bundle-registry/status", &[]).await;
        assert_eq!(plain.status(), StatusCode::OK);
        assert!(plain.headers().get("access-control-allow-origin").is_none());
        let cross = send(
            &app,
            "GET",
            "/asset-bundle-registry/status",
            &[("origin", "https://example.org")],
        )
        .await;
        assert_eq!(cross.status(), StatusCode::OK);
        assert_eq!(cross.headers()["access-control-allow-origin"], "*");
        assert!(
            cross
                .headers()
                .get("access-control-expose-headers")
                .is_none()
        );

        let other = send(
            &app,
            "OPTIONS",
            "/realm-provider-ea/main/about",
            &request_headers,
        )
        .await;
        assert_eq!(other.status(), StatusCode::OK);
        assert_eq!(other.headers()["access-control-allow-methods"], "*");
        assert_eq!(other.headers()["access-control-allow-headers"], "*");
    }

    /// The Registry's CORS runs before its routing: a path no route serves
    /// still answers the captured preflight, and its 404 (the captured
    /// unknown-route answer) gets origin `*` for a cross-origin request.
    #[tokio::test]
    async fn registry_cors_covers_unrouted_paths() {
        let preflight: Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/content-lambdas/captures/abr/AB-12-preflight.json"
        )))
        .unwrap();
        let unknown: Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/content-lambdas/captures/abr/AB-12-unknown-route.json"
        )))
        .unwrap();
        let request_headers: Vec<(&str, &str)> = preflight["request"]["headers"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str().unwrap()))
            .collect();
        let app = app().await;
        for path in ["/asset-bundle-registry/nope", "/asset-bundle-registry"] {
            let response = send(&app, "OPTIONS", path, &request_headers).await;
            assert_eq!(response.status(), StatusCode::NO_CONTENT, "{path}");
            let headers: serde_json::Map<String, Value> = response
                .headers()
                .iter()
                .filter(|(name, _)| name.as_str().starts_with("access-control-") || *name == "vary")
                .map(|(name, value)| (name.to_string(), Value::from(value.to_str().unwrap())))
                .collect();
            assert_eq!(
                Value::Object(headers),
                preflight["response"]["headers"],
                "{path}"
            );
        }
        for origin in [None, Some("https://example.org")] {
            let headers: Vec<(&str, &str)> = origin.map(|o| ("origin", o)).into_iter().collect();
            let response = send(&app, "GET", "/asset-bundle-registry/nope", &headers).await;
            assert_eq!(
                response.status().as_u16() as u64,
                unknown["response"]["status"].as_u64().unwrap()
            );
            assert_eq!(
                response.headers()["content-type"],
                unknown["response"]["headers"]["content-type"]
                    .as_str()
                    .unwrap()
            );
            assert_eq!(
                response
                    .headers()
                    .get("access-control-allow-origin")
                    .map(|value| value.to_str().unwrap()),
                origin.map(|_| "*"),
                "{origin:?}"
            );
            assert_eq!(
                to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
                unknown["response"]["body"].as_str().unwrap().as_bytes()
            );
        }
    }

    /// `@dcl/http-server` answers every `OPTIONS` as a preflight: without
    /// `Origin` it still sends origin `*` (`cors.js` `configureOrigin`), and
    /// without `Access-Control-Request-Headers` it keeps the `Vary` and sends
    /// no `Allow-Headers` (`configureAllowedHeaders`).
    #[tokio::test]
    async fn registry_preflight_without_origin_or_requested_headers() {
        let app = app().await;
        let path = "/asset-bundle-registry/entities/active";
        for (headers, allow_headers) in [
            (
                vec![
                    ("access-control-request-method", "POST"),
                    ("access-control-request-headers", "content-type"),
                ],
                Some("content-type"),
            ),
            (
                vec![
                    ("origin", "https://example.org"),
                    ("access-control-request-method", "POST"),
                ],
                None,
            ),
            (vec![], None),
        ] {
            let response = send(&app, "OPTIONS", path, &headers).await;
            assert_eq!(response.status(), StatusCode::NO_CONTENT, "{headers:?}");
            let h = response.headers();
            assert_eq!(h["access-control-allow-origin"], "*", "{headers:?}");
            assert_eq!(
                h["access-control-allow-methods"], "GET,HEAD,OPTIONS,DELETE,POST,PUT",
                "{headers:?}"
            );
            assert_eq!(h["vary"], "Access-Control-Request-Headers", "{headers:?}");
            assert_eq!(h["access-control-max-age"], "86400", "{headers:?}");
            assert_eq!(
                h.get("access-control-allow-headers")
                    .map(|value| value.to_str().unwrap()),
                allow_headers,
                "{headers:?}"
            );
            assert!(h.get("access-control-allow-credentials").is_none());
        }
    }

    /// `createStatusCheckComponent`'s probes under the Registry prefix; its
    /// only probe-capable component is the http server it names `server`.
    #[tokio::test]
    async fn registry_answers_the_status_check_probes() {
        let pass = r#"{"details":{"server":{"status":"pass"}},"status":"pass"}"#;
        let app = app().await;
        for (path, content_type, body) in [
            ("/health/live", "text/plain; charset=utf-8", "alive"),
            (
                "/health/ready",
                "application/health+json; charset=utf-8",
                pass,
            ),
            (
                "/health/startup",
                "application/health+json; charset=utf-8",
                pass,
            ),
        ] {
            let response = send(&app, "GET", &format!("/asset-bundle-registry{path}"), &[]).await;
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(response.headers()["content-type"], content_type, "{path}");
            assert_eq!(
                to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
                body.as_bytes(),
                "{path}"
            );
        }
    }
}
