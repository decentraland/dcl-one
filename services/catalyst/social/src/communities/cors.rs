//! The social-api CORS answers, as production's `@dcl/http-server` server
//! (`src/components.ts` at `b6b2cef`: `cors: { methods, maxAge: 86400 }`)
//! sends them: any origin, reflected preflight request headers, no
//! credentials. A request without `Origin` gets no CORS headers, as the
//! captures without one show. The shared `cors_middleware` reflects origins
//! and allows credentials for the other services; this layer replaces it on
//! the communities router only.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

const ALLOW_METHODS: &str = "GET,HEAD,OPTIONS,DELETE,POST,PUT,PATCH";
const MAX_AGE: &str = "86400";

pub async fn social_api_cors(req: Request, next: Next) -> Response {
    // The source's Node adapter rejects TRACE while constructing its native
    // Request, before CORS runs. Node's parser rejects TRACK even earlier.
    if matches!(req.method().as_str(), "TRACE" | "TRACK") {
        let trace = req.method().as_str() == "TRACE";
        let mut response = Response::builder()
            .status(if req.method().as_str() == "TRACK" {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            })
            .body(Body::empty())
            .unwrap();
        if trace {
            response
                .headers_mut()
                .insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
        }
        return response;
    }
    let has_origin = req.headers().contains_key(header::ORIGIN);
    let preflight = req.method() == Method::OPTIONS;
    let requested_headers = req
        .headers()
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .cloned();
    let mut resp = if preflight {
        let mut resp = Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap();
        let h = resp.headers_mut();
        h.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static(ALLOW_METHODS),
        );
        if let Some(value) = requested_headers
            && !value.as_bytes().is_empty()
        {
            h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, value);
        }
        h.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static(MAX_AGE),
        );
        h.insert(
            header::VARY,
            HeaderValue::from_static("Access-Control-Request-Headers"),
        );
        resp
    } else {
        next.run(req).await
    };
    let h = resp.headers_mut();
    if has_origin || preflight {
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
