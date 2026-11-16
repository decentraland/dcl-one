//! The social-api CORS answers, as production's `@dcl/http-server` server
//! (`src/components.ts` at `b6b2cef`: `cors: { methods, maxAge: 86400 }`)
//! sends them ([`HttpServerCors`]): any origin, reflected preflight request
//! headers, no credentials. A request without `Origin` other than a preflight
//! gets no CORS headers, as the captures without one show. The shared `cors_middleware` reflects origins
//! and allows credentials for the other services; this layer replaces it on
//! the communities router only. It keeps that layer's
//! `X-Content-Type-Options: nosniff`, which the source does not send, so
//! user uploads such as community thumbnails are never content-sniffed.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

use crate::shared::cors::HttpServerCors;

const CORS: HttpServerCors = HttpServerCors {
    methods: Some("GET,HEAD,OPTIONS,DELETE,POST,PUT,PATCH"),
    max_age: "86400",
    nosniff: true,
    preflight_echoes_request: false,
};

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
        return nosniff(response);
    }
    CORS.run(req, next).await
}

/// `X-Content-Type-Options: nosniff` on a Social answer, also for the
/// gateway's answers that leave before CORS (`axum::middleware::map_response`).
pub async fn content_type_options(response: Response) -> Response {
    nosniff(response)
}

fn nosniff(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}
