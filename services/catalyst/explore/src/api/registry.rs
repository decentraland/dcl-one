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

use std::sync::LazyLock;
use std::time::Duration;

use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
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

pub fn routes() -> Router<Shared> {
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
