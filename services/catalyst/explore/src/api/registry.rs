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
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
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
        .route("/entities/active", post(boot_read))
        .route("/entities/versions", post(boot_read))
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

/// The methods the Registry's `createServerComponent` lists (`components.ts`).
const CORS_METHODS: &str = "GET,HEAD,OPTIONS,DELETE,POST,PUT";

/// The Registry's CORS (`@dcl/http-server` `cors.js` with `methods` and
/// `maxAge: 86400`, captured as `captures/abr/AB-12-preflight.json`): every
/// `OPTIONS` is a 204 preflight with origin `*`, the listed methods, the
/// requested headers reflected (`Vary: Access-Control-Request-Headers`) and
/// a one-day max-age; any other response gets origin `*` only when the
/// request carries an `Origin`, and never exposed headers.
pub async fn cors(req: Request, next: Next) -> Response {
    if req.method() == Method::OPTIONS {
        let mut response = StatusCode::NO_CONTENT.into_response();
        let h = response.headers_mut();
        h.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
        h.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static(CORS_METHODS),
        );
        h.insert(
            header::VARY,
            HeaderValue::from_static("Access-Control-Request-Headers"),
        );
        if let Some(requested) = req
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
            .filter(|value| !value.is_empty())
        {
            h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, requested.clone());
        }
        h.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static("86400"),
        );
        return response;
    }
    let has_origin = req.headers().contains_key(header::ORIGIN);
    let mut response = next.run(req).await;
    if has_origin {
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
    }
    response
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

/// `/entities/active` and `/entities/versions`: unity-explorer calls them at
/// boot, so a node without a Registry knows no converted entity (`200 []`).
async fn boot_read(
    state: State<Shared>,
    original_uri: OriginalUri,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if state.settings.sources.registry.is_empty() {
        return Json(json!([])).into_response();
    }
    read(state, original_uri, method, uri, headers, body).await
}

async fn read(
    State(state): State<Shared>,
    OriginalUri(original_uri): OriginalUri,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let source = &state.settings.sources.registry;
    if source.is_empty() {
        return message(
            StatusCode::SERVICE_UNAVAILABLE,
            "Registry service is not configured",
        );
    }
    let (native, mut url) = match backend(&state.settings, source) {
        Ok(backend) => backend,
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "Registry source refused");
            return message(
                StatusCode::SERVICE_UNAVAILABLE,
                "Registry service is not configured",
            );
        }
    };
    let path = format!("{}{}", url.path().trim_end_matches('/'), uri.path());
    url.set_path(&path);
    if url.path() != path {
        // Encoded dot segments the URL parser would normalize out of the route.
        return message(StatusCode::BAD_REQUEST, "Invalid Registry path");
    }
    url.set_query(uri.query());
    let Some(_permit) = slot(&PERMITS, SLOT_WAIT).await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [("retry-after", "1")],
            Json(json!({"ok":false,"message":"Registry service is busy"})),
        )
            .into_response();
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
        for (name, value) in &headers {
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
        Ok::<_, anyhow::Error>((status, out_headers, bytes).into_response())
    }
    .await;
    result.unwrap_or_else(|error| {
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
