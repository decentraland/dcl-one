//! The per-route HTTP series the content server and lamb2 emitted through
//! `@dcl/http-server` (`instrumentHttpServerWithPromClientRegistry`):
//! `http_requests_total`, `http_request_duration_seconds` and
//! `http_request_size_bytes`, each labelled `method`, `handler` and `code`.
//!
//! `handler` is the route template those services matched, in
//! `path-to-regexp` syntax (`/contents/:hashId`), after their reverse proxy
//! stripped the `/content` or `/lambdas` prefix: the root and `/content`
//! mounts of a content route share one label. A request no route matched has
//! an empty `handler`, as theirs did. The duration ends when the handler
//! returns its response, before a streamed body is sent, and the size is the
//! request's `Content-Length` (0 without one). The histogram buckets are set
//! where the exporter is built (`comms::metrics::prometheus_builder`).

use axum::extract::{MatchedPath, Request};
use axum::http::{Method, header};
use axum::middleware::Next;
use axum::response::Response;

pub const REQUESTS: &str = "http_requests_total";
pub const DURATION: &str = "http_request_duration_seconds";
pub const SIZE: &str = "http_request_size_bytes";

/// The help text `@dcl/http-server` declares for the three series.
pub fn register_series() {
    metrics::describe_counter!(REQUESTS, "Total number of HTTP requests");
    metrics::describe_histogram!(DURATION, "Request duration in seconds.");
    metrics::describe_histogram!(SIZE, "Duration of HTTP requests size in bytes");
}

/// Records the three series for a content or lamb2 route. Apply it as the
/// outermost layer of each router serving those routes, so a timeout answer
/// is counted too.
pub async fn observe(request: Request, next: Next) -> Response {
    let handler = match request.extensions().get::<MatchedPath>() {
        Some(matched) => legacy_handler(matched.as_str()),
        // The content router's fallback answers every unmatched path; the
        // content server only saw the ones under `/content/`.
        None if request.uri().path().starts_with("/content/") => Some(String::new()),
        None => None,
    };
    let Some(handler) = handler else {
        return next.run(request).await;
    };
    let method = method_label(request.method());
    let size = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(0.0);
    let started = std::time::Instant::now();
    let response = next.run(request).await;
    let labels = [
        ("method", method),
        ("handler", handler),
        ("code", response.status().as_u16().to_string()),
    ];
    metrics::histogram!(SIZE, &labels).record(size);
    metrics::counter!(REQUESTS, &labels).increment(1);
    metrics::histogram!(DURATION, &labels).record(started.elapsed().as_secs_f64());
    response
}

/// The standard methods by name; anything else shares one bounded label.
fn method_label(method: &Method) -> String {
    match method.as_str() {
        "GET" | "POST" | "PUT" | "DELETE" | "OPTIONS" | "HEAD" | "PATCH" | "TRACE" | "CONNECT" => {
            method.as_str().to_owned()
        }
        _ => "other".to_owned(),
    }
}

/// The `handler` label the content server or lamb2 gave the route an axum
/// template names, or `None` for a route outside both: the trades reads the
/// Shop router serves beside the lambdas (`/v1/...`).
pub fn legacy_handler(template: &str) -> Option<String> {
    if template.starts_with("/v1/") {
        return None;
    }
    let route = match strip_mount(template, "/lambdas") {
        // The gateway's 404 under `/lambdas/`: no lamb2 route matched.
        Some("/{*rest}") => return Some(String::new()),
        // The old proxy sent these to lamb2's and the content server's own paths.
        Some("/collections") => "/nfts/collections",
        Some("/collections/contents/{pointer}/thumbnail") => "/queries/items/{pointer}/thumbnail",
        Some("/collections/contents/{pointer}/image") => "/queries/items/{pointer}/image",
        Some(route) => route,
        None => strip_mount(template, "/content").unwrap_or(template),
    };
    // One content server route with an optional last segment.
    if route.starts_with("/queries/erc721/") {
        return Some("/queries/erc721/:chainId/:contract/:option/:emission?".to_owned());
    }
    Some(
        route
            .split('/')
            .map(
                |segment| match segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    Some(name) => format!(":{}", name.trim_start_matches('*')),
                    None => segment.to_owned(),
                },
            )
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// `template` below the `mount` segment, which must end there.
fn strip_mount<'a>(template: &'a str, mount: &str) -> Option<&'a str> {
    template
        .strip_prefix(mount)
        .filter(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::legacy_handler;

    #[test]
    fn handlers_are_the_legacy_route_templates() {
        for (template, legacy) in [
            ("/contents/{hashId}", "/contents/:hashId"),
            ("/content/contents/{hashId}", "/contents/:hashId"),
            (
                "/content/contents/{hashId}/active-entities",
                "/contents/:hashId/active-entities",
            ),
            ("/content/entities/{type}", "/entities/:type"),
            ("/entities", "/entities"),
            ("/content/audit/{type}/{entityId}", "/audit/:type/:entityId"),
            (
                "/content/queries/erc721/{chainId}/{contract}/{option}",
                "/queries/erc721/:chainId/:contract/:option/:emission?",
            ),
            (
                "/queries/erc721/{chainId}/{contract}/{option}/{emission}",
                "/queries/erc721/:chainId/:contract/:option/:emission?",
            ),
            ("/lambdas/profiles", "/profiles"),
            ("/lambdas/profiles/{id}", "/profiles/:id"),
            (
                "/lambdas/users/{address}/third-party-wearables/{collectionId}",
                "/users/:address/third-party-wearables/:collectionId",
            ),
            (
                "/lambdas/users/{address}/parcels/{x}/{y}/permissions",
                "/users/:address/parcels/:x/:y/permissions",
            ),
            ("/lambdas/collections", "/nfts/collections"),
            (
                "/lambdas/collections/contents/{pointer}/image",
                "/queries/items/:pointer/image",
            ),
            ("/lambdas/about", "/about"),
            ("/about", "/about"),
            (
                "/explorer/{address}/wearables",
                "/explorer/:address/wearables",
            ),
            ("/lambdas/{*rest}", ""),
            // A mount is a whole segment.
            ("/lambdasx/a", "/lambdasx/a"),
            ("/contentx/{id}", "/contentx/:id"),
        ] {
            assert_eq!(
                legacy_handler(template).as_deref(),
                Some(legacy),
                "{template}"
            );
        }
        assert_eq!(legacy_handler("/v1/trades/{id}"), None);
    }
}
