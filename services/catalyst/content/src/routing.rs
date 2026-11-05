//! What upstream's router (the koa-router port in `@dcl/http-server`) does
//! around the handlers and axum does not: case-insensitive static segments,
//! its `Allow` list on a 405, and the body cap's 413 text. Each answer is
//! pinned by a production capture under
//! `tests/fixtures/conformance/content-lambdas/captures/content-reads/`.

use axum::Router;
use axum::extract::Request;
use axum::http::header::{ALLOW, CONTENT_TYPE};
use axum::http::{HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};

/// The read paths `content_routes` registers, rewritten case-insensitively;
/// every registered path is here or in [`CASE_SENSITIVE_ROUTE_PATHS`]
/// (`every_content_route_is_listed_once` in the crate tests checks both).
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
];

/// The write and admin paths, kept case-sensitive: the captures cover reads
/// only, and the gateway's deploy and admin locations match these spellings.
#[cfg(test)]
pub(crate) const CASE_SENSITIVE_ROUTE_PATHS: &[&str] = &[
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
    let read = matches!(*req.method(), Method::GET | Method::HEAD);
    let Some(path) = canonical_path(req.uri().path(), read) else {
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
/// `/entities/active`: a `GET` or `HEAD` (`read`) keeps it for
/// `/entities/{type}`, which echoes `Active` in its error as upstream does,
/// and any other method takes `/entities/active`, upstream's `POST` route.
pub(crate) fn canonical_path(path: &str, read: bool) -> Option<String> {
    let rest = path.strip_prefix(CONTENT_PREFIX)?;
    if !rest.bytes().any(|b| b.is_ascii_uppercase()) {
        return None;
    }
    let mut segments: Vec<&str> = rest.split('/').collect();
    let mut changed = false;
    for route in CONTENT_ROUTE_PATHS {
        if read && *route == "/entities/active" {
            continue;
        }
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
            canonical_path(&format!("/content/Contents/{cid}"), true),
            Some(format!("/content/contents/{cid}"))
        );
        assert_eq!(
            canonical_path(&format!("/content/CONTENTS/{cid}/Active-Entities"), true),
            Some(format!("/content/contents/{cid}/active-entities"))
        );
        assert_eq!(
            canonical_path("/content/Status", true),
            Some("/content/status".into())
        );
        assert_eq!(
            canonical_path("/content/Deployments", true),
            Some("/content/deployments".into())
        );
        assert_eq!(
            canonical_path("/content/Entities/Scene", true),
            Some("/content/entities/Scene".into())
        );
        assert_eq!(
            canonical_path("/content/entities/Active", false),
            Some("/content/entities/active".into()),
            "a POST takes upstream's /entities/active"
        );
        assert_eq!(
            canonical_path("/content/Entities/Active", true),
            Some("/content/entities/Active".into()),
            "a GET keeps the type for /entities/{{type}}"
        );
        assert_eq!(
            canonical_path("/content/Entities/Active/Collections/urn:A", true),
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
            "/content/Entities",
            "/content/Scenes/0,0",
            "/content/Forwarding",
        ] {
            assert_eq!(canonical_path(path, true), None, "{path}");
            assert_eq!(canonical_path(path, false), None, "{path}");
        }
    }

    #[test]
    fn allow_lists_head_get_post_with_a_space() {
        assert_eq!(upstream_allow_value("GET,HEAD"), "HEAD, GET");
        assert_eq!(upstream_allow_value("GET,HEAD,POST"), "HEAD, GET, POST");
        assert_eq!(upstream_allow_value("POST"), "POST");
        assert_eq!(upstream_allow_value("DELETE"), "DELETE");
    }
}
