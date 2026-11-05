//! What upstream's router (the koa-router port in `@dcl/http-server`) does
//! around the handlers and axum does not: case-insensitive static segments,
//! its `Allow` list on a 405, and the body cap's 413 text. Each answer is
//! pinned by a production capture under
//! `tests/fixtures/conformance/content-lambdas/captures/content-reads/`.

use axum::Router;
use axum::extract::Request;
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_EXPOSE_HEADERS, ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS, ALLOW,
    CONTENT_TYPE, ORIGIN, VARY, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};

/// Every path `content_routes` registers; keep the two in step (the test
/// below checks each one is routed).
pub(crate) const CONTENT_ROUTE_PATHS: &[&str] = &[
    "/challenge",
    "/entities/{type}",
    "/entities/active/collections/{collectionUrn}",
    "/entities/active",
    "/contents/{hashId}",
    "/contents/{hashId}/active-entities",
    "/available-content",
    "/audit/{type}/{entityId}",
    "/deployments",
    "/failed-deployments",
    "/pointer-changes",
    "/snapshots",
    "/snapshot",
    "/snapshot/{type}",
    "/status",
    "/queries/items/{pointer}/thumbnail",
    "/queries/items/{pointer}/image",
    "/queries/erc721/{chainId}/{contract}/{option}",
    "/queries/erc721/{chainId}/{contract}/{option}/{emission}",
    "/entities",
    "/scenes/{coord}",
    "/forwarding",
    "/forwarding/{entityId}",
    "/forwarding/{entityId}/retry",
];

/// Upstream production serves the content routes only under this prefix.
const CONTENT_PREFIX: &str = "/content/";

/// Upstream's routing for the paths under `/content/`, around the finished
/// app: a rewrite inside a router runs after that router has chosen its
/// route, and axum's `MethodRouter` adds `Allow` outside its route layers.
///
/// - `/content/Status`, `/content/Contents/<hash>` and the like are routed as
///   their lower-case routes, as upstream's router (`sensitive: false`) does
///   (`CR-16-path-case-*.json`, `CS-23-path-case-deployments.json`). Only the
///   static segments of a content route are rewritten: parameters (hashes,
///   URNs) keep their case.
/// - A 405 lists `Allow` as upstream does ([`upstream_allow`]).
///
/// The root mounts and every other service's paths are left alone.
pub fn upstream_routing(app: Router) -> Router {
    Router::new()
        .fallback_service(app)
        .layer(middleware::from_fn(upstream_allow))
        .layer(middleware::map_request(|req: Request| async move {
            canonical_case(req)
        }))
}

fn canonical_case(mut req: Request) -> Request {
    let Some(path) = canonical_path(req.uri().path()) else {
        return req;
    };
    let path_and_query = match req.uri().query() {
        Some(query) => format!("{path}?{query}"),
        None => path,
    };
    let mut parts = req.uri().clone().into_parts();
    if let Ok(path_and_query) = path_and_query.parse() {
        parts.path_and_query = Some(path_and_query);
        if let Ok(uri) = Uri::from_parts(parts) {
            *req.uri_mut() = uri;
        }
    }
    req
}

fn is_param(segment: &str) -> bool {
    segment.starts_with('{')
}

/// The path with the static segments of every content route it matches
/// (ignoring case) spelled as the route spells them; `None` when nothing
/// changes. `/entities/Active` matches both `/entities/{type}` and
/// `/entities/active`, and becomes the latter, the route upstream's `POST`
/// takes.
pub(crate) fn canonical_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix(CONTENT_PREFIX)?;
    let mut segments: Vec<&str> = rest.split('/').collect();
    let mut changed = false;
    for route in CONTENT_ROUTE_PATHS {
        let route: Vec<&str> = route.trim_start_matches('/').split('/').collect();
        if route.len() != segments.len()
            || !route
                .iter()
                .zip(&segments)
                .all(|(r, s)| is_param(r) || r.eq_ignore_ascii_case(s))
        {
            continue;
        }
        for (r, s) in route.iter().zip(segments.iter_mut()) {
            if !is_param(r) && *r != *s {
                *s = *r;
                changed = true;
            }
        }
    }
    changed.then(|| format!("{CONTENT_PREFIX}{}", segments.join("/")))
}

/// A 405's `Allow` as koa-router writes it: `HEAD, GET, POST`, comma and
/// space, `HEAD` first (`CR-12-put-status.json`, `CR-12-delete-entities-active.json`);
/// axum writes `GET,HEAD`.
async fn upstream_allow(req: Request, next: Next) -> Response {
    let under_content = req.uri().path().starts_with(CONTENT_PREFIX);
    let mut res = next.run(req).await;
    if under_content
        && res.status() == StatusCode::METHOD_NOT_ALLOWED
        && let Some(allow) = res.headers().get(ALLOW).and_then(|v| v.to_str().ok())
        && let Ok(allow) = HeaderValue::from_str(&upstream_allow_value(allow))
    {
        res.headers_mut().insert(ALLOW, allow);
    }
    res
}

fn upstream_allow_value(allow: &str) -> String {
    // koa-router registers `get` as HEAD then GET, and lists the matched
    // routes in registration order; every upstream content route is HEAD,
    // GET or POST. dcl-one's own write routes follow.
    const ORDER: [&str; 6] = ["HEAD", "GET", "POST", "PUT", "DELETE", "PATCH"];
    let methods: Vec<&str> = allow
        .split(',')
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .collect();
    let mut out: Vec<&str> = ORDER.into_iter().filter(|m| methods.contains(m)).collect();
    for m in methods {
        if !out.contains(&m) {
            out.push(m);
        }
    }
    out.join(", ")
}

/// Upstream's preflight methods (`decentraland/catalyst:src/components.ts:492-505`).
const UPSTREAM_CORS_METHODS: &str = "GET,HEAD,POST,OPTIONS";

/// The CORS rule of the content server under `/content/`, as upstream
/// configures `@dcl/http-server` (`decentraland/catalyst:src/components.ts:492-505`;
/// `CR-11-preflight.json`, `CR-11-get-with-origin.json`): any origin as `*`
/// without credentials, every header exposed (a handler's own list kept),
/// and a preflight that allows `GET,HEAD,POST,OPTIONS`, reflects the requested
/// headers, lasts 600 s and varies on `Access-Control-Request-Headers`. A
/// request without `Origin` gets no CORS header, as captured.
///
/// The root mounts, the lambdas routes in this router and this node's own
/// write routes under `/content/` (`scenes`, `forwarding`, which need
/// `DELETE`) keep the shared rule.
pub(crate) async fn content_cors(req: Request, next: Next) -> Response {
    if !upstream_cors_path(req.uri().path()) {
        return crate::cors::cors_middleware(req, next).await;
    }
    let has_origin = req.headers().contains_key(ORIGIN);
    if req.method() == Method::OPTIONS {
        let mut res = StatusCode::NO_CONTENT.into_response();
        let requested = req.headers().get(ACCESS_CONTROL_REQUEST_HEADERS).cloned();
        let h = res.headers_mut();
        if has_origin {
            h.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
            h.insert(ACCESS_CONTROL_EXPOSE_HEADERS, HeaderValue::from_static("*"));
        }
        h.insert(
            ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static(UPSTREAM_CORS_METHODS),
        );
        if let Some(requested) = requested {
            h.insert(ACCESS_CONTROL_ALLOW_HEADERS, requested);
        }
        h.insert(ACCESS_CONTROL_MAX_AGE, HeaderValue::from_static("600"));
        h.insert(
            VARY,
            HeaderValue::from_static("Access-Control-Request-Headers"),
        );
        h.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
        return res;
    }
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    if has_origin {
        h.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
        if !h.contains_key(ACCESS_CONTROL_EXPOSE_HEADERS) {
            h.insert(ACCESS_CONTROL_EXPOSE_HEADERS, HeaderValue::from_static("*"));
        }
    }
    h.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    res
}

fn upstream_cors_path(path: &str) -> bool {
    path.strip_prefix(CONTENT_PREFIX).is_some_and(|rest| {
        let first = rest.split('/').next().unwrap_or("");
        !first.eq_ignore_ascii_case("scenes") && !first.eq_ignore_ascii_case("forwarding")
    })
}

/// `POST /entities/active`'s body cap, upstream's `MAX_ACTIVE_ENTITIES_BODY_SIZE`
/// default (`decentraland/catalyst:src/Environment.ts:79`).
pub(crate) const MAX_ACTIVE_ENTITIES_BODY_BYTES: usize = 10 * 1024 * 1024;

/// A body over the cap answers upstream's `413 text/plain` `Payload Too Large`
/// (`CR-17-body-over-10mib.json`) instead of axum's rejection text.
pub(crate) async fn payload_too_large_text(req: Request, next: Next) -> Response {
    let res = next.run(req).await;
    if res.status() != StatusCode::PAYLOAD_TOO_LARGE {
        return res;
    }
    (
        StatusCode::PAYLOAD_TOO_LARGE,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        "Payload Too Large",
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_segments_take_the_route_case_and_parameters_keep_theirs() {
        let cid = "bafkreiAbC";
        assert_eq!(
            canonical_path(&format!("/content/Contents/{cid}")),
            Some(format!("/content/contents/{cid}"))
        );
        assert_eq!(
            canonical_path(&format!("/content/CONTENTS/{cid}/Active-Entities")),
            Some(format!("/content/contents/{cid}/active-entities"))
        );
        assert_eq!(
            canonical_path("/content/Status"),
            Some("/content/status".into())
        );
        assert_eq!(
            canonical_path("/content/Deployments"),
            Some("/content/deployments".into())
        );
        assert_eq!(
            canonical_path("/content/Entities/Scene"),
            Some("/content/entities/Scene".into())
        );
        assert_eq!(
            canonical_path("/content/entities/Active"),
            Some("/content/entities/active".into())
        );
        assert_eq!(
            canonical_path("/content/Entities/Active/Collections/urn:A"),
            Some("/content/entities/active/collections/urn:A".into())
        );
    }

    #[test]
    fn already_canonical_unknown_and_non_content_paths_are_left_alone() {
        for path in [
            "/content/status",
            "/content/contents/bafkreiAbC",
            "/content/Nope",
            "/content/Status/extra",
            "/Status",
            "/Content/Status",
            "/lambdas/Status",
            "/shop-api/Status",
        ] {
            assert_eq!(canonical_path(path), None, "{path}");
        }
    }

    #[test]
    fn allow_lists_head_get_post_with_a_space() {
        assert_eq!(upstream_allow_value("GET,HEAD"), "HEAD, GET");
        assert_eq!(upstream_allow_value("GET,HEAD,POST"), "HEAD, GET, POST");
        assert_eq!(upstream_allow_value("POST"), "POST");
        assert_eq!(upstream_allow_value("DELETE"), "DELETE");
    }

    fn cors_app() -> Router {
        use axum::routing::get;
        Router::new()
            .fallback(get(|| async { "ok" }).delete(|| async { "gone" }))
            .layer(middleware::from_fn(content_cors))
    }

    async fn cors_headers(method: Method, path: &str, origin: bool) -> axum::http::HeaderMap {
        use tower::ServiceExt;
        let mut req = Request::builder().method(method).uri(path);
        if origin {
            req = req
                .header(ORIGIN, "https://example.org")
                .header(ACCESS_CONTROL_REQUEST_HEADERS, "x-identity-auth-chain-0");
        }
        let res = cors_app()
            .oneshot(req.body(axum::body::Body::empty()).unwrap())
            .await
            .unwrap();
        res.headers().clone()
    }

    /// CR-11: under `/content/` upstream's rule, as captured.
    #[tokio::test]
    async fn content_paths_answer_upstreams_cors() {
        let h = cors_headers(Method::GET, "/content/status", true).await;
        assert_eq!(h[ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert_eq!(h[ACCESS_CONTROL_EXPOSE_HEADERS], "*");
        assert!(h.get("access-control-allow-credentials").is_none());
        assert!(h.get(VARY).is_none());

        let h = cors_headers(Method::GET, "/content/status", false).await;
        assert!(h.get(ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        assert!(h.get(ACCESS_CONTROL_EXPOSE_HEADERS).is_none());

        let h = cors_headers(Method::OPTIONS, "/content/entities/active", true).await;
        assert_eq!(h[ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert_eq!(h[ACCESS_CONTROL_ALLOW_METHODS], "GET,HEAD,POST,OPTIONS");
        assert_eq!(h[ACCESS_CONTROL_ALLOW_HEADERS], "x-identity-auth-chain-0");
        assert_eq!(h[ACCESS_CONTROL_MAX_AGE], "600");
        assert_eq!(h[VARY], "Access-Control-Request-Headers");
    }

    /// The root mounts and this node's `DELETE /content/scenes/{coord}` keep
    /// the shared rule, whose preflight allows `DELETE`.
    #[tokio::test]
    async fn other_paths_keep_the_shared_cors() {
        for path in ["/status", "/content/scenes/0,0", "/content/Forwarding"] {
            let h = cors_headers(Method::OPTIONS, path, true).await;
            assert_eq!(
                h[ACCESS_CONTROL_ALLOW_ORIGIN], "https://example.org",
                "{path}"
            );
            assert!(
                h[ACCESS_CONTROL_ALLOW_METHODS]
                    .to_str()
                    .unwrap()
                    .contains("DELETE"),
                "{path}"
            );
        }
    }
}
