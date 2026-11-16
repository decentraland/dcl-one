//! Hand-rolled CORS: an origin-less request gets no CORS headers at all
//! (`tower_http::CorsLayer` would emit them), a preflight short-circuits to
//! 204 without a registered OPTIONS handler, and the allowed headers are
//! REFLECTED from `Access-Control-Request-Headers` because signed-fetch auth
//! chains are open-ended (`X-Identity-Auth-Chain-N` grows with delegation and
//! EIP-1654 links), so any enumerated list is a ceiling that breaks login.
//! Decision 2026-07-31 (transparent-front cutover); the cors fixtures assert it.
//! [`HttpServerCors`] is the `@dcl/http-server` rule of three surfaces: the
//! Registry gateway, the social API and `/lambdas/`. The content routes keep
//! their own `content_cors` (`content/src/routing.rs`), which also exposes
//! headers and is not covered by it.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

const ALLOW_METHODS: &str = "GET,HEAD,POST,PUT,DELETE,PATCH,OPTIONS";

const ALLOW_HEADERS: &str = "Cache-Control,Content-Type,Origin,Accept,User-Agent,X-Upload-Origin,Range,If-None-Match,If-Modified-Since,X-Identity-Timestamp,X-Identity-Metadata,X-Identity-Auth-Chain-0,X-Identity-Auth-Chain-1,X-Identity-Auth-Chain-2,X-Identity-Auth-Chain-3";
const MAX_AGE: &str = "86400";

pub async fn cors_middleware(req: Request, next: Next) -> Response {
    let origin = req.headers().get(header::ORIGIN).cloned();
    let is_preflight = req.method() == Method::OPTIONS;
    let requested_headers = req
        .headers()
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .cloned();

    if is_preflight {
        let mut resp = Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap();
        let h = resp.headers_mut();
        h.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static(ALLOW_METHODS),
        );
        h.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            requested_headers.unwrap_or(HeaderValue::from_static(ALLOW_HEADERS)),
        );
        h.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static(MAX_AGE),
        );
        h.insert(header::VARY, HeaderValue::from_static("Origin"));
        if let Some(origin) = origin {
            allow_origin(&mut resp, origin);
        }
        add_security_headers(&mut resp);
        return resp;
    }

    let mut resp = next.run(req).await;

    if let Some(origin) = origin {
        allow_origin(&mut resp, origin);
        let h = resp.headers_mut();
        h.insert(header::VARY, HeaderValue::from_static("Origin"));
        if !h.contains_key(header::ACCESS_CONTROL_EXPOSE_HEADERS) {
            h.insert(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                HeaderValue::from_static("*"),
            );
        }
    }

    add_security_headers(&mut resp);
    resp
}

fn allow_origin(resp: &mut Response, origin: HeaderValue) {
    let h = resp.headers_mut();
    h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    h.insert(
        header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
        HeaderValue::from_static("true"),
    );
}

fn add_security_headers(resp: &mut Response) {
    resp.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
}

/// `@dcl/http-server`'s CORS (`dist/cors.js` in 2.1.0 and 2.2.1) for a
/// service that sets `cors: { methods?, maxAge }` and no `origin`. Every
/// `OPTIONS` is a 204 preflight with origin `*`, the configured methods,
/// `Vary: Access-Control-Request-Headers`, those headers reflected when the
/// request names any, and the max-age; it does not depend on `Origin`. Any
/// other answer gets origin `*` only when the request carries `Origin`, and
/// no credentials or exposed headers.
#[derive(Clone, Copy, Debug)]
pub struct HttpServerCors {
    /// `Access-Control-Allow-Methods` on a preflight (`methods.join(',')`);
    /// `None` when the service lists no methods, as the source then sends none.
    pub methods: Option<&'static str>,
    /// `Access-Control-Max-Age` on a preflight, in seconds.
    pub max_age: &'static str,
    /// Adds `X-Content-Type-Options: nosniff` to every answer, which the
    /// source does not send.
    pub nosniff: bool,
    /// A preflight sends origin `*` only for a request with `Origin`, and
    /// the reflected headers and their `Vary` only for a request with
    /// `Access-Control-Request-Headers` (reflected even when empty), instead
    /// of the source's unconditional origin and `Vary`.
    pub preflight_echoes_request: bool,
}

impl HttpServerCors {
    /// Runs the rule around `next`; a preflight never reaches it.
    pub async fn run(self, req: Request, next: Next) -> Response {
        let has_origin = req.headers().contains_key(header::ORIGIN);
        let mut response = if req.method() == Method::OPTIONS {
            self.preflight(req.headers())
        } else {
            let mut response = next.run(req).await;
            if has_origin {
                response.headers_mut().insert(
                    header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    HeaderValue::from_static("*"),
                );
            }
            response
        };
        if self.nosniff {
            add_security_headers(&mut response);
        }
        response
    }

    fn preflight(self, request: &HeaderMap) -> Response {
        let requested = request.get(header::ACCESS_CONTROL_REQUEST_HEADERS);
        let mut response = StatusCode::NO_CONTENT.into_response();
        let h = response.headers_mut();
        if !self.preflight_echoes_request || request.contains_key(header::ORIGIN) {
            h.insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
        }
        if let Some(methods) = self.methods {
            h.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static(methods),
            );
        }
        let reflected = if self.preflight_echoes_request {
            requested
        } else {
            requested.filter(|value| !value.is_empty())
        };
        if reflected.is_some() || !self.preflight_echoes_request {
            h.insert(
                header::VARY,
                HeaderValue::from_static("Access-Control-Request-Headers"),
            );
        }
        if let Some(reflected) = reflected {
            h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, reflected.clone());
        }
        h.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static(self.max_age),
        );
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::routing::get;
    use tower::ServiceExt;

    fn app() -> Router {
        Router::new()
            .route("/x", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(cors_middleware))
    }

    fn req(method: Method, origin: Option<&str>) -> Request {
        let mut b = Request::builder().method(method).uri("/x");
        if let Some(o) = origin {
            b = b.header(header::ORIGIN, o);
        }
        b.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn no_origin_emits_no_cors_headers() {
        let resp = app().oneshot(req(Method::GET, None)).await.unwrap();
        let h = resp.headers();
        assert!(h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        assert!(h.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none());
        assert!(h.get(header::VARY).is_none());
        assert_eq!(h.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff");
    }

    #[tokio::test]
    async fn origin_is_reflected_with_credentials_and_vary() {
        let resp = app()
            .oneshot(req(Method::GET, Some("https://example.org")))
            .await
            .unwrap();
        let h = resp.headers();
        assert_eq!(
            h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
            "https://example.org"
        );
        assert_eq!(
            h.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).unwrap(),
            "true"
        );
        assert_eq!(h.get(header::ACCESS_CONTROL_EXPOSE_HEADERS).unwrap(), "*");
        assert_eq!(h.get(header::VARY).unwrap(), "Origin");
    }

    #[tokio::test]
    async fn handler_chosen_expose_headers_survive() {
        let app = Router::new()
            .route(
                "/x",
                get(|| async { ([(header::ACCESS_CONTROL_EXPOSE_HEADERS, "ETag")], "ok") }),
            )
            .layer(axum::middleware::from_fn(cors_middleware));
        let resp = app
            .oneshot(req(Method::GET, Some("https://example.org")))
            .await
            .unwrap();
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_EXPOSE_HEADERS)
                .unwrap(),
            "ETag"
        );
    }

    #[tokio::test]
    async fn preflight_is_204_and_reflects_the_requested_headers() {
        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/x")
            .header(header::ORIGIN, "https://example.org")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
            .body(Body::empty())
            .unwrap();
        let resp = app().oneshot(request).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        let h = resp.headers();
        assert_eq!(
            h.get(header::ACCESS_CONTROL_ALLOW_METHODS).unwrap(),
            "GET,HEAD,POST,PUT,DELETE,PATCH,OPTIONS"
        );
        assert_eq!(
            h.get(header::ACCESS_CONTROL_ALLOW_HEADERS).unwrap(),
            "content-type"
        );
        assert_eq!(h.get(header::ACCESS_CONTROL_MAX_AGE).unwrap(), "86400");
        assert_eq!(
            h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
            "https://example.org"
        );
        assert_eq!(
            h.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).unwrap(),
            "true"
        );
        assert_eq!(h.get(header::VARY).unwrap(), "Origin");
    }

    #[tokio::test]
    async fn preflight_without_requested_headers_advertises_the_default_list() {
        let resp = app()
            .oneshot(req(Method::OPTIONS, Some("https://example.org")))
            .await
            .unwrap();
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                .unwrap(),
            ALLOW_HEADERS
        );
    }

    #[tokio::test]
    async fn preflight_without_origin_still_204() {
        let resp = app().oneshot(req(Method::OPTIONS, None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
        assert_eq!(resp.headers().get(header::VARY).unwrap(), "Origin");
    }

    const REGISTRY: HttpServerCors = HttpServerCors {
        methods: Some("GET,HEAD,OPTIONS,DELETE,POST,PUT"),
        max_age: "86400",
        nosniff: false,
        preflight_echoes_request: false,
    };

    fn http_server_app(cors: HttpServerCors) -> Router {
        Router::new()
            .route("/x", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |req, next| {
                cors.run(req, next)
            }))
    }

    async fn http_server_preflight(cors: HttpServerCors, headers: &[(&str, &str)]) -> HeaderMap {
        let mut request = Request::builder().method(Method::OPTIONS).uri("/x");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let resp = http_server_app(cors)
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        resp.headers().clone()
    }

    /// `cors.js` `configureOrigin` sets `*` on every preflight when no
    /// `origin` is configured, with or without a request `Origin`.
    #[tokio::test]
    async fn http_server_preflight_without_origin_still_allows_any_origin() {
        let h = http_server_preflight(
            REGISTRY,
            &[("access-control-request-headers", "content-type")],
        )
        .await;
        assert_eq!(h[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert_eq!(
            h[header::ACCESS_CONTROL_ALLOW_METHODS],
            "GET,HEAD,OPTIONS,DELETE,POST,PUT"
        );
        assert_eq!(h[header::ACCESS_CONTROL_ALLOW_HEADERS], "content-type");
        assert_eq!(h[header::ACCESS_CONTROL_MAX_AGE], "86400");
        assert_eq!(h[header::VARY], "Access-Control-Request-Headers");
        assert!(h.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none());
        assert!(h.get(header::X_CONTENT_TYPE_OPTIONS).is_none());
    }

    /// `configureAllowedHeaders` always sets the `Vary` and sends
    /// `Allow-Headers` only for a non-empty request list.
    #[tokio::test]
    async fn http_server_preflight_without_requested_headers_keeps_the_vary() {
        for requested in [None, Some("")] {
            let mut headers = vec![("origin", "https://example.org")];
            headers.extend(requested.map(|value| ("access-control-request-headers", value)));
            let h = http_server_preflight(REGISTRY, &headers).await;
            assert_eq!(h[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*", "{requested:?}");
            assert_eq!(
                h[header::VARY],
                "Access-Control-Request-Headers",
                "{requested:?}"
            );
            assert!(
                h.get(header::ACCESS_CONTROL_ALLOW_HEADERS).is_none(),
                "{requested:?}"
            );
            assert_eq!(h[header::ACCESS_CONTROL_MAX_AGE], "86400", "{requested:?}");
        }
    }

    /// The echoing variant sends the origin and the reflected headers only
    /// for what the request carries, and no methods when none are listed.
    #[tokio::test]
    async fn http_server_preflight_that_echoes_the_request() {
        let cors = HttpServerCors {
            methods: None,
            max_age: "36000",
            nosniff: true,
            preflight_echoes_request: true,
        };
        let h = http_server_preflight(cors, &[]).await;
        for absent in [
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            header::ACCESS_CONTROL_ALLOW_METHODS,
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            header::VARY,
        ] {
            assert!(h.get(&absent).is_none(), "{absent}");
        }
        assert_eq!(h[header::ACCESS_CONTROL_MAX_AGE], "36000");
        assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        let h = http_server_preflight(
            cors,
            &[
                ("origin", "https://example.org"),
                ("access-control-request-headers", ""),
            ],
        )
        .await;
        assert_eq!(h[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert_eq!(h[header::ACCESS_CONTROL_ALLOW_HEADERS], "");
        assert_eq!(h[header::VARY], "Access-Control-Request-Headers");
    }

    /// An answer that is not a preflight gets `*` only for a request with
    /// `Origin`, and never credentials, exposed headers or a `Vary`.
    #[tokio::test]
    async fn http_server_answer_gets_the_star_only_with_an_origin() {
        for nosniff in [false, true] {
            let cors = HttpServerCors {
                nosniff,
                ..REGISTRY
            };
            for origin in [None, Some("https://example.org")] {
                let resp = http_server_app(cors)
                    .oneshot(req(Method::GET, origin))
                    .await
                    .unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
                let h = resp.headers();
                assert_eq!(
                    h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                        .map(|v| v.to_str().unwrap()),
                    origin.map(|_| "*"),
                );
                assert!(h.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none());
                assert!(h.get(header::ACCESS_CONTROL_EXPOSE_HEADERS).is_none());
                assert!(h.get(header::VARY).is_none());
                assert_eq!(h.contains_key(header::X_CONTENT_TYPE_OPTIONS), nosniff);
            }
        }
    }
}
