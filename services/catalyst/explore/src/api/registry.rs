//! Registry gateway. Content/profile authority stays in Catalyst; conversion
//! operations are served by the configured local abgen runtime without a cache.

use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use bytes::Bytes;
use serde_json::{Value, json};

use super::Shared;

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/entities/active", post(native))
        .route("/entities/versions", post(native))
        .route("/entities/status", get(native))
        .route("/entities/status/{id}", get(native))
        .route("/worlds/{world_name}/manifest", get(native))
        .route("/denylist", get(native))
        .route("/denylist/{entity_id}", post(native).delete(native))
        .route("/queues/status", get(native))
        .route("/queues/retry", post(native))
        .route("/queues/pause", post(native))
        .route("/queues/resume", post(native))
        .route("/registry", post(native))
        .route("/flush-cache", delete(native))
        .route("/status", get(status))
        .route("/health", get(health))
}

pub(crate) fn backend_url(base: &str) -> anyhow::Result<url::Url> {
    let url = url::Url::parse(base)?;
    let local = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback() || ip.is_private(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback() || ip.is_unique_local(),
        Some(url::Host::Domain(host)) => host == "localhost" || host == "catalyst-abgen",
        None => false,
    };
    anyhow::ensure!(
        local
            && url.scheme() == "http"
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Registry backend must be the local abgen HTTP service"
    );
    Ok(url)
}

async fn native(
    State(state): State<Shared>,
    OriginalUri(original_uri): OriginalUri,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if state.settings.sources.registry.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"ok":false,"message":"Registry service is not configured"})),
        )
            .into_response();
    }
    let result = async {
        let mut url = backend_url(&state.settings.sources.registry)?;
        let path = format!("{}{}", url.path().trim_end_matches('/'), uri.path());
        url.set_path(&path);
        anyhow::ensure!(url.path() == path, "Registry route must not be normalized");
        url.set_query(uri.query());
        let mut request = state
            .http
            .clone()
            .allowing_configured_provider_hosts()
            .request(method, url)
            .max_redirects(0)
            .timeout(std::time::Duration::from_secs(60))
            .body(body);
        for (name, value) in &headers {
            if matches!(
                name.as_str(),
                "content-type" | "accept" | "authorization" | "if-none-match" | "if-modified-since"
            ) || name.as_str().starts_with("x-identity-")
            {
                request = request.header(name, value);
            }
        }
        // Never trust a caller-supplied path. The native verifier checks this
        // actual URI received by Catalyst against the forwarded route. A gateway
        // may have added the Registry prefix before this request arrived.
        request = request.header("x-original-path", original_uri.to_string());
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
                bytes.len().saturating_add(chunk.len()) <= 64 << 20,
                "Registry response exceeds body limit"
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok::<_, anyhow::Error>((status, out_headers, bytes).into_response())
    }
    .await;
    result.unwrap_or_else(|error| {
        tracing::warn!(%error, "local Registry request failed");
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"ok":false,"message":"Registry service unavailable"})),
        )
            .into_response()
    })
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
