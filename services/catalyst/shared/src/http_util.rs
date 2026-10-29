//! The lambdas error envelope the marketplace, LAND and price routes answer with.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// The lambdas error envelope: `{"error": "<status text>", "message": "<detail>"}`
/// (the old `UpstreamErrorBody`), distinct from the content server's `{"error"}`.
pub fn lambdas_error(status: StatusCode, label: &str, message: &str) -> Response {
    (status, Json(json!({ "error": label, "message": message }))).into_response()
}

pub fn bad_request(message: &str) -> Response {
    lambdas_error(StatusCode::BAD_REQUEST, "Bad request", message)
}

pub fn not_found(message: &str) -> Response {
    lambdas_error(StatusCode::NOT_FOUND, "Not Found", message)
}

pub fn service_unavailable(message: &str) -> Response {
    lambdas_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Service Unavailable",
        message,
    )
}

pub fn internal_server_error() -> Response {
    lambdas_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal Server Error",
        "Internal Server Error",
    )
}
