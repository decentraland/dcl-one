//! The social-api CORS answers, as production's `@dcl/http-server` server
//! (`src/components.ts` at `fb3dcb8`: `cors: { methods, maxAge: 86400 }`)
//! sends them (captures `social/preflight-communities` and
//! `social/communities-list-origin`): any origin, every header, no
//! credentials. A request without `Origin` gets no CORS headers, as the
//! captures without one show. The shared `cors_middleware` reflects origins
//! and allows credentials for the other services; this layer replaces it on
//! the communities router only.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

const ALLOW_METHODS: &str = "GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS";
const MAX_AGE: &str = "86400";
const VARY: &str = "accept-encoding";

pub async fn social_api_cors(req: Request, next: Next) -> Response {
    let has_origin = req.headers().contains_key(header::ORIGIN);
    let mut resp = if req.method() == Method::OPTIONS {
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
            HeaderValue::from_static("*"),
        );
        h.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static(MAX_AGE),
        );
        h.insert(header::VARY, HeaderValue::from_static(VARY));
        resp
    } else {
        let mut resp = next.run(req).await;
        if has_origin {
            resp.headers_mut()
                .insert(header::VARY, HeaderValue::from_static(VARY));
        }
        resp
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
