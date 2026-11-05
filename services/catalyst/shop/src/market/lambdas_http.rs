//! The HTTP edges of the `/lambdas/*` routes in [`super::router`], as the
//! production peer answers them (`captures/lambdas-profiles/LP-31-*.json`):
//! lamb2's http-server CORS (`cors: { maxAge: 36000 }`) instead of the
//! node's shared one, and the gateway's nginx 404 page for a `/lambdas/`
//! path no route matches (an unknown name or another case). The router's
//! other paths (`/outfits`, `/explorer`, `/v1/trades`) keep the shared CORS.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::shared::cors;

const MAX_AGE: &str = "36000";

/// The page the peer's nginx answers for a path it does not route to lamb2.
const NGINX_NOT_FOUND: &str = "<html>\n<head><title>404 Not Found</title></head>\n<body>\n<center><h1>404 Not Found</h1></center>\n<hr><center>nginx</center>\n</body>\n</html>\n";

/// lamb2's CORS under `/lambdas/`: `*` for any origin, no credentials and
/// no exposed headers; a preflight is 204 with the requested headers
/// reflected and no `Allow-Methods`. Every other path gets the shared CORS.
pub async fn cors_middleware(req: Request, next: Next) -> Response {
    if !req.uri().path().starts_with("/lambdas/") {
        return cors::cors_middleware(req, next).await;
    }
    let has_origin = req.headers().contains_key(header::ORIGIN);
    let mut resp = if req.method() == Method::OPTIONS {
        let requested_headers = req
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
            .cloned();
        let mut resp = Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap();
        let h = resp.headers_mut();
        if let Some(requested) = requested_headers {
            h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, requested);
            h.insert(
                header::VARY,
                HeaderValue::from_static("Access-Control-Request-Headers"),
            );
        }
        h.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static(MAX_AGE),
        );
        resp
    } else {
        next.run(req).await
    };
    let h = resp.headers_mut();
    if has_origin {
        h.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
    }
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    resp
}

/// `/lambdas/{*rest}`: a path under `/lambdas/` that no route matches.
pub async fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/html")],
        NGINX_NOT_FOUND,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::tests::support::state_without_db;
    use tower::ServiceExt;

    async fn send(request: Request) -> Response {
        crate::market::router(state_without_db())
            .oneshot(request)
            .await
            .unwrap()
    }

    /// `LP-31-preflight.json`: 204, `*`, the requested headers, max-age 36000
    /// and `Vary: Access-Control-Request-Headers`, nothing else.
    #[tokio::test]
    async fn a_lambdas_preflight_answers_as_lamb2() {
        let resp = send(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/lambdas/profiles")
                .header(header::ORIGIN, "https://example.org")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        let h = resp.headers();
        assert_eq!(h[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert_eq!(h[header::ACCESS_CONTROL_ALLOW_HEADERS], "content-type");
        assert_eq!(h[header::ACCESS_CONTROL_MAX_AGE], "36000");
        assert_eq!(h[header::VARY], "Access-Control-Request-Headers");
        for absent in [
            header::ACCESS_CONTROL_ALLOW_METHODS,
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
        ] {
            assert!(h.get(&absent).is_none(), "{absent}");
        }
    }

    /// `LP-31-get-with-origin.json`: only `Access-Control-Allow-Origin: *`.
    #[tokio::test]
    async fn a_lambdas_answer_with_an_origin_carries_only_the_star() {
        let resp = send(
            Request::get("/lambdas/status")
                .header(header::ORIGIN, "https://example.org")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let h = resp.headers();
        assert_eq!(h[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert!(h.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none());
        assert!(h.get(header::ACCESS_CONTROL_EXPOSE_HEADERS).is_none());
        assert!(h.get(header::VARY).is_none());
        let resp = send(Request::get("/lambdas/status").body(Body::empty()).unwrap()).await;
        assert!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
    }

    /// The router's paths outside `/lambdas/` keep the node's shared CORS.
    #[tokio::test]
    async fn other_paths_keep_the_shared_cors() {
        let resp = send(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/v1/trades")
                .header(header::ORIGIN, "https://example.org")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        let h = resp.headers();
        assert_eq!(
            h[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://example.org"
        );
        assert_eq!(h[header::ACCESS_CONTROL_ALLOW_CREDENTIALS], "true");
        assert_eq!(h[header::ACCESS_CONTROL_MAX_AGE], "86400");
    }

    /// `LP-31-put-profiles.json`, `LP-31-get-profiles.json`: 405, empty body, `Allow: POST`.
    #[tokio::test]
    async fn a_wrong_method_is_an_empty_405() {
        for method in [Method::PUT, Method::GET] {
            let resp = send(
                Request::builder()
                    .method(method.clone())
                    .uri("/lambdas/profiles")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await;
            assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED, "{method}");
            assert_eq!(resp.headers()[header::ALLOW], "POST");
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            assert!(body.is_empty(), "{method}");
        }
    }

    /// `LP-31-unknown-route.json`, `LP-31-path-case-*.json`: the nginx page.
    #[tokio::test]
    async fn an_unrouted_lambdas_path_answers_the_gateway_404() {
        for uri in [
            "/lambdas/nope",
            "/lambdas/Status",
            "/lambdas/Profiles/0x0000000000000000000000000000000000000001",
        ] {
            let resp = send(Request::get(uri).body(Body::empty()).unwrap()).await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri}");
            assert_eq!(resp.headers()[header::CONTENT_TYPE], "text/html");
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(body, NGINX_NOT_FOUND.as_bytes(), "{uri}");
        }
    }

    /// The catch-all only answers what no merged router routes: another
    /// router's `/lambdas/` paths (content's `/lambdas/about`, LAND's
    /// parcel routes) still reach their handlers.
    #[tokio::test]
    async fn the_catch_all_yields_to_other_routers() {
        use axum::routing::get;
        let other = axum::Router::new()
            .route("/lambdas/about", get(|| async { "about" }))
            .route(
                "/lambdas/users/{address}/lands-permissions",
                get(|| async { "permissions" }),
            )
            .route(
                "/lambdas/collections/contents/{pointer}/thumbnail",
                get(|| async { "thumbnail" }),
            );
        let app = other.merge(crate::market::router(state_without_db()));
        for (uri, expected) in [
            ("/lambdas/about", "about"),
            ("/lambdas/users/0x1/lands-permissions", "permissions"),
            ("/lambdas/collections/contents/urn:x/thumbnail", "thumbnail"),
        ] {
            let resp = app
                .clone()
                .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "{uri}");
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(body, expected.as_bytes());
        }
    }
}
