//! Cross-origin headers as camera-reel-service's actix-cors 0.6.5 answers
//! them, matched against the dated production preflight capture
//! (`tests/fixtures/conformance/social/camera-reel/preflight-user.json`).
//!
//! `/api` (any origin, any header, methods `DELETE, GET, POST, PATCH`,
//! max-age 300, every response header exposed) and `/health` (any origin,
//! any header, `GET, OPTIONS`, max-age 3600). The OpenAPI document and the
//! Swagger page under `/api/docs/` carry none, as upstream, and neither do
//! the admin routes.
//!
//! A preflight is an `OPTIONS` with a parseable
//! `Access-Control-Request-Method`: 200 with an empty body, the origin
//! echoed, the method list, the requested headers echoed, the max-age and
//! `Vary`; a requested method outside the list is 400
//! `Requested method is not allowed`. Every other request reaches the
//! router and its answer gets `Vary`, the echoed origin when the request
//! had one, and on `/api` `Access-Control-Expose-Headers` listing the
//! answer's own header names but `content-length`. [`layer`] wraps the
//! whole router, the writes gate included, so a 503 refusal and a 404
//! carry the headers too.

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};

const VARY: &str = "Origin, Access-Control-Request-Method, Access-Control-Request-Headers";

#[derive(Debug, Clone, Copy, PartialEq)]
struct Policy {
    methods: &'static [Method],
    allow_methods: &'static str,
    max_age: &'static str,
    expose_any: bool,
}

const API: Policy = Policy {
    methods: &[Method::DELETE, Method::GET, Method::POST, Method::PATCH],
    allow_methods: "DELETE, GET, POST, PATCH",
    max_age: "300",
    expose_any: true,
};

const HEALTH: Policy = Policy {
    methods: &[Method::GET, Method::OPTIONS],
    allow_methods: "GET, OPTIONS",
    max_age: "3600",
    expose_any: false,
};

/// The policy for a route-local path (the router as `serve` nests it).
fn policy(path: &str) -> Option<Policy> {
    if path == "/api/docs/openapi.json"
        || path == "/api/docs/ui"
        || path.starts_with("/api/docs/ui/")
    {
        return None;
    }
    if path == "/api" || path.starts_with("/api/") {
        return Some(API);
    }
    if path == "/health" || path.starts_with("/health/") {
        return Some(HEALTH);
    }
    None
}

fn add_vary(headers: &mut HeaderMap) {
    let value = match headers.get(header::VARY) {
        Some(existing) => {
            let mut v = existing.as_bytes().to_vec();
            v.extend_from_slice(b", ");
            v.extend_from_slice(VARY.as_bytes());
            HeaderValue::from_bytes(&v).unwrap_or(HeaderValue::from_static(VARY))
        }
        None => HeaderValue::from_static(VARY),
    };
    headers.insert(header::VARY, value);
}

fn requested_method(req: &Request) -> Option<Method> {
    let value = req.headers().get(header::ACCESS_CONTROL_REQUEST_METHOD)?;
    Method::from_bytes(value.as_bytes()).ok()
}

fn preflight(policy: Policy, req: &Request, method: Method) -> Response {
    if !policy.methods.contains(&method) {
        return (StatusCode::BAD_REQUEST, "Requested method is not allowed").into_response();
    }
    let mut res = StatusCode::OK.into_response();
    let headers = res.headers_mut();
    if let Some(origin) = req.headers().get(header::ORIGIN) {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin.clone());
    }
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static(policy.allow_methods),
    );
    if let Some(requested) = req.headers().get(header::ACCESS_CONTROL_REQUEST_HEADERS) {
        headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, requested.clone());
    }
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static(policy.max_age),
    );
    add_vary(headers);
    res
}

async fn cors(req: Request, next: Next) -> Response {
    let Some(policy) = policy(req.uri().path()) else {
        return next.run(req).await;
    };
    if req.method() == Method::OPTIONS
        && let Some(method) = requested_method(&req)
    {
        return preflight(policy, &req, method);
    }
    let origin = req.headers().get(header::ORIGIN).cloned();
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    if let Some(origin) = origin {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    if policy.expose_any {
        // actix adds `content-length` when it writes the body, after the
        // middleware; the captures' expose lists never name it.
        let names = headers
            .keys()
            .filter(|name| *name != header::CONTENT_LENGTH)
            .map(|name| name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if !names.is_empty()
            && let Ok(value) = HeaderValue::from_str(&names)
        {
            headers.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, value);
        }
    }
    add_vary(headers);
    res
}

/// Wraps every route and the fallback of `router` in the policy its path
/// selects.
pub fn layer(router: Router) -> Router {
    router.layer(middleware::from_fn(cors))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_docs_and_admin_paths_carry_no_cors() {
        assert_eq!(policy("/api/users/0x1"), Some(API));
        assert_eq!(policy("/api/does-not-exist"), Some(API));
        assert_eq!(policy("/health/live"), Some(HEALTH));
        assert_eq!(policy("/api/docs/openapi.json"), None);
        assert_eq!(policy("/api/docs/ui/"), None);
        assert_eq!(policy("/admin/images/x"), None);
        assert_eq!(policy("/apix"), None);
    }
}
