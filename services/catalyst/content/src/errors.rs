//! One error type for the content routes. Every error renders as the
//! `{"error": "<message>"}` envelope the upstream content server emits (the
//! old `ApiErrorBody` `{ok,error,message}` shape was the lambdas contract and
//! is not used here); internal errors never leak their detail.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    InvalidRequest(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Unauthorized(String),

    #[error("{0}")]
    Forbidden(String),

    #[error("{0}")]
    Conflict(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        AppError::InvalidRequest(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        AppError::NotFound(msg.into())
    }

    fn status_and_message(&self) -> (StatusCode, String) {
        match self {
            AppError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            AppError::Internal(detail) => {
                tracing::error!(detail, "internal error on a content route");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error".to_string(),
                )
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = self.status_and_message();
        json_error(status, &message)
    }
}

pub fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn is_nul_byte_db_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("0x00")
        || m.contains("22021")
        || m.contains("nul byte")
        || m.contains("nul character")
        || m.contains("null byte")
        || (m.contains("invalid") && m.contains("utf") && m.contains("00"))
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        let text = e.to_string();
        if is_nul_byte_db_error(&text) {
            AppError::InvalidRequest("a request value contains an invalid NUL byte".to_string())
        } else {
            AppError::Internal(text)
        }
    }
}

impl From<crate::storage::StorageError> for AppError {
    fn from(e: crate::storage::StorageError) -> Self {
        AppError::Internal(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = AppError::not_found("Entity not found").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v, json!({ "error": "Entity not found" }));
    }

    #[tokio::test]
    async fn internal_errors_do_not_expose_details() {
        let response = AppError::Internal("private database details".into()).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), br#"{"error":"Internal Server Error"}"#);
    }

    #[test]
    fn write_side_variants_keep_their_status_codes() {
        assert_eq!(
            AppError::Unauthorized("no".into()).into_response().status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::Conflict("taken".into()).into_response().status(),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn nul_byte_db_errors_are_recognized() {
        assert!(is_nul_byte_db_error(
            "sqlx error: error returned from database: invalid byte sequence for encoding \"UTF8\": 0x00"
        ));
        assert!(is_nul_byte_db_error(
            "encode error: unexpected NUL byte in string"
        ));
        assert!(is_nul_byte_db_error("SQLSTATE 22021"));
    }

    #[test]
    fn ordinary_db_errors_are_not_nul() {
        assert!(!is_nul_byte_db_error("sqlx error: pool timed out"));
        assert!(!is_nul_byte_db_error("connection refused"));
        assert!(!is_nul_byte_db_error(
            "relation \"deployments\" does not exist"
        ));
    }
}
