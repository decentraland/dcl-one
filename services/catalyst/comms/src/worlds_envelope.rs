//! The worlds content server's error envelope: `{ok: false, error, message}`
//! (the lambdas-style `ApiErrorBody` the old service answered on every
//! non-deploy route), with the two upload-limit shapes (`{error, message}`
//! with 503 + `Retry-After` and 408) the multipart routes share with
//! `POST /entities`. Internal detail never reaches the wire. Shared by
//! worlds (its routes, through `worlds::core::http`) and the world comms
//! mints in comms, which answer in the worlds service's envelope.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header::RETRY_AFTER};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{message}")]
    Http { status: u16, message: String },

    #[error("internal error")]
    Internal(String),

    #[error("{message}")]
    TooManyRequests { message: String, retry_after: u64 },

    /// 503 with `Retry-After: 5`: shed by the shared multipart upload limiter.
    #[error("{0}")]
    UploadShed(String),

    /// 408: multipart receive or processing deadline exceeded.
    #[error("{0}")]
    RequestTimeout(String),
}

impl ApiError {
    pub fn http(status: u16, msg: impl Into<String>) -> Self {
        Self::Http {
            status,
            message: msg.into(),
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
    /// 409: a concurrent deploy changed the overlapping-scene set out from under a
    /// scoped replacement authorization.
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::http(409, msg)
    }
    pub fn too_many(msg: impl Into<String>, retry_after: u64) -> Self {
        Self::TooManyRequests {
            message: msg.into(),
            retry_after,
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
    pub fn service_unavailable(msg: impl Into<String>) -> Self {
        Self::http(503, msg)
    }
    pub fn is_conflict(&self) -> bool {
        matches!(self, ApiError::Http { status: 409, .. })
    }
    pub fn is_bad_request(&self) -> bool {
        matches!(self, ApiError::Http { status: 400, .. })
    }
    pub fn is_forbidden(&self) -> bool {
        matches!(self, ApiError::Http { status: 403, .. })
    }
    pub fn status(&self) -> u16 {
        match self {
            ApiError::Http { status, .. } => *status,
            ApiError::Internal(_) => 500,
            ApiError::TooManyRequests { .. } => 429,
            ApiError::UploadShed(_) => 503,
            ApiError::RequestTimeout(_) => 408,
        }
    }
    /// The message the wire carries (internal detail stays in the log).
    pub fn public_message(&self) -> String {
        match self {
            ApiError::Http { message, .. } => message.clone(),
            ApiError::Internal(_) => "internal error".to_string(),
            ApiError::TooManyRequests { message, .. } => message.clone(),
            ApiError::UploadShed(m) | ApiError::RequestTimeout(m) => m.clone(),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(format!("database: {e}"))
    }
}

impl From<crate::shared::identity::AuthorityNotEstablished> for ApiError {
    fn from(why: crate::shared::identity::AuthorityNotEstablished) -> Self {
        use crate::shared::identity::AuthorityNotEstablished as A;
        match &why {
            A::UndeterminedStoreUnavailable {
                store,
                reason_for_operators,
            } => {
                tracing::warn!(store, reason = %reason_for_operators, "authority undetermined");
                Self::http(503, why.to_string())
            }
            _ => Self::http(why.http_status(), why.to_string()),
        }
    }
}

pub fn envelope(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({ "ok": false, "error": message, "message": message })),
    )
        .into_response()
}

/// The `{error, message}` body of the shed and timeout answers (the deploy
/// route's rejection shape, not the worlds envelope).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct DeployRejection {
    pub error: String,
    pub message: String,
}

pub fn shed_response(message: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [("Retry-After", "5")],
        Json(DeployRejection {
            error: "Service Unavailable".to_string(),
            message: message.to_string(),
        }),
    )
        .into_response()
}

pub fn timeout_response(message: &str) -> Response {
    (
        StatusCode::REQUEST_TIMEOUT,
        Json(DeployRejection {
            error: "Request Timeout".to_string(),
            message: message.to_string(),
        }),
    )
        .into_response()
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = match self {
            ApiError::Internal(detail) => {
                tracing::error!(detail, "internal error on a worlds route");
                envelope(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
            }
            ApiError::Http { status, message } => envelope(
                StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                &message,
            ),
            ApiError::TooManyRequests {
                message,
                retry_after,
            } => (
                StatusCode::TOO_MANY_REQUESTS,
                [("Retry-After", retry_after.to_string())],
                Json(json!({ "ok": false, "error": message, "message": message })),
            )
                .into_response(),
            ApiError::UploadShed(m) => shed_response(&m),
            ApiError::RequestTimeout(m) => timeout_response(&m),
        };
        if response.status() == StatusCode::SERVICE_UNAVAILABLE
            && !response.headers().contains_key(RETRY_AFTER)
        {
            response
                .headers_mut()
                .insert(RETRY_AFTER, HeaderValue::from_static("1"));
        }
        response
    }
}

/// `clamp_limit(requested, default, max)` of the old catalyrst-types.
pub fn clamp_limit(requested: Option<i64>, default: i64, max: i64) -> i64 {
    requested.unwrap_or(default).clamp(1, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn undetermined_world_token_responses_preserve_envelope_and_request_retry() {
        let authority = crate::shared::identity::AuthorityNotEstablished::store_unavailable(
            "worlds.worlds",
            &sqlx::Error::PoolTimedOut,
        );
        let error = ApiError::from(authority);
        let expected = error.public_message();
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()[RETRY_AFTER], "1");
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            json!({"ok": false, "error": expected, "message": expected})
        );
        assert!(
            !ApiError::forbidden("denied")
                .into_response()
                .headers()
                .contains_key(RETRY_AFTER)
        );
        assert_eq!(
            ApiError::UploadShed("busy".into())
                .into_response()
                .headers()[RETRY_AFTER],
            "5"
        );
        assert_eq!(
            ApiError::too_many("busy", 7).into_response().headers()[RETRY_AFTER],
            "7"
        );
    }

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = ApiError::not_found("world not found").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "error": "world not found", "message": "world not found" })
        );
    }

    #[tokio::test]
    async fn internal_detail_is_not_published() {
        let resp = ApiError::internal("local content open: /srv/worlds/secret.bin").into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "error": "internal error", "message": "internal error" })
        );
    }

    #[test]
    fn clamp_limit_applies_default_and_bounds() {
        assert_eq!(clamp_limit(None, 500, 5000), 500);
        assert_eq!(clamp_limit(Some(0), 500, 5000), 1);
        assert_eq!(clamp_limit(Some(-9), 500, 5000), 1);
        assert_eq!(clamp_limit(Some(42), 500, 5000), 42);
        assert_eq!(clamp_limit(Some(1_000_000), 500, 5000), 5000);
    }

    #[test]
    fn an_undetermined_authority_is_503_and_a_refusal_403() {
        let refused = crate::shared::identity::AuthorityNotEstablished::RefusedLacksAuthority {
            detail: "no".into(),
        };
        assert_eq!(ApiError::from(refused).status(), 403);
        let undetermined =
            crate::shared::identity::AuthorityNotEstablished::UndeterminedStoreUnavailable {
                store: "worlds.worlds",
                reason_for_operators: "down".into(),
            };
        assert_eq!(ApiError::from(undetermined).status(), 503);
        assert!(ApiError::conflict("x").is_conflict());
    }
}
