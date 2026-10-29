//! Hand-rolled CORS: an origin-less request gets no CORS headers at all
//! (`tower_http::CorsLayer` would emit them), a preflight short-circuits to
//! 204 without a registered OPTIONS handler, and the allowed headers are
//! REFLECTED from `Access-Control-Request-Headers` because signed-fetch auth
//! chains are open-ended (`X-Identity-Auth-Chain-N` grows with delegation and
//! EIP-1654 links), so any enumerated list is a ceiling that breaks login.
//! Decision 2026-07-31 (transparent-front cutover); the cors fixtures assert it.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

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
}
