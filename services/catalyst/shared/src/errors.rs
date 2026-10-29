//! The explore routes' error and success envelopes. Errors render as the
//! lambdas-style `{"ok":false,"error":msg,"message":msg,"code":...}` body the
//! old places service emitted (a superset of the events one, which had no
//! `code`); database and internal errors never leak their detail.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{message}")]
    Http { status: u16, message: String },

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("{0}")]
    Internal(String),

    /// A write refused by the node's write switch (`explore::writes`), or
    /// held by its restore stamps (`writes::refusal`): 503 with
    /// `Retry-After`, the social switch's answer in this envelope.
    #[error("{0}")]
    WritesDisabled(std::borrow::Cow<'static, str>),
}

impl ApiError {
    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::Http {
            status,
            message: message.into(),
        }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::http(400, msg)
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::http(401, msg)
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::http(403, msg)
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::http(404, msg)
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::http(409, msg)
    }
    pub fn service_unavailable(msg: impl Into<String>) -> Self {
        Self::http(503, msg)
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

fn code_for_status(status: u16) -> &'static str {
    match status {
        400 => "bad_request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not_found",
        409 => "conflict",
        429 => "too_many_requests",
        500 => "internal_server_error",
        501 => "not_implemented",
        503 => "service_unavailable",
        _ => "error",
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::WritesDisabled(message) => {
                let body = json!({
                    "ok": false,
                    "error": message,
                    "message": message,
                    "code": code_for_status(503),
                });
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    [(axum::http::header::RETRY_AFTER, "3600")],
                    Json(body),
                )
                    .into_response();
            }
            ApiError::Http { status, message } => (status, message),
            ApiError::Database(err) => {
                tracing::error!(error = %err, "sqlx error on an explore route");
                (500, "database error".to_string())
            }
            ApiError::Internal(m) => {
                tracing::error!(error = %m, "internal error on an explore route");
                (500, "internal error".to_string())
            }
        };
        let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = json!({
            "ok": false,
            "error": message,
            "message": message,
            "code": code_for_status(status),
        });
        (status_code, Json(body)).into_response()
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct ApiData<T> {
    pub ok: bool,
    pub data: T,
}

impl<T: Serialize> ApiData<T> {
    pub fn ok(data: T) -> Self {
        Self { ok: true, data }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct ApiDataTotal<T> {
    pub ok: bool,
    pub data: Vec<T>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
}

impl<T: Serialize> ApiDataTotal<T> {
    pub fn ok(data: Vec<T>, total: i64) -> Self {
        Self {
            ok: true,
            data,
            total,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct ApiDataTotalMap<T> {
    pub ok: bool,
    pub data: std::collections::BTreeMap<String, T>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
}

impl<T: Serialize> ApiDataTotalMap<T> {
    pub fn ok(data: std::collections::BTreeMap<String, T>, total: i64) -> Self {
        Self {
            ok: true,
            data,
            total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn error_envelope_carries_both_message_keys_and_a_code() {
        let resp = ApiError::not_found("Not found place \"x\"").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({
                "ok": false,
                "error": "Not found place \"x\"",
                "message": "Not found place \"x\"",
                "code": "not_found"
            })
        );
    }

    #[tokio::test]
    async fn database_errors_do_not_expose_details() {
        let resp = ApiError::Database(sqlx::Error::PoolClosed).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["message"], "database error");
    }

    #[test]
    fn wire_identity_api_ok_envelope() {
        let v = serde_json::to_value(ApiData::ok(vec![1, 2])).unwrap();
        assert_eq!(v, json!({ "ok": true, "data": [1, 2] }));
        let v = serde_json::to_value(ApiDataTotal::ok(vec!["a"], 7)).unwrap();
        assert_eq!(v, json!({ "ok": true, "data": ["a"], "total": 7 }));
    }
}
