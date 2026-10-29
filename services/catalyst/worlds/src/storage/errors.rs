//! The world-storage error envelope: `{"ok": false, "error": <label>,
//! "message": <text>}` (the old `ApiErrorBody`), with the upstream
//! worlds-content-server status codes. A signed-fetch refusal carries its raw
//! chain error as `error` and the fixed ADR-44 sentence as `message`; an
//! internal fault never leaks its detail.

use axum::Json;
use axum::http::{StatusCode, header::RETRY_AFTER};
use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::shared::identity::AuthorityNotEstablished;

pub const SIGNED_FETCH_MESSAGE: &str = "This endpoint requires a signed fetch request. See ADR-44.";

#[derive(Debug, serde::Serialize)]
struct ErrorBody {
    ok: bool,
    error: String,
    message: String,
}

impl ErrorBody {
    fn new(message: &str) -> Self {
        Self {
            ok: false,
            error: message.to_string(),
            message: message.to_string(),
        }
    }

    fn labeled(error: &str, message: &str) -> Self {
        Self {
            ok: false,
            error: error.to_string(),
            message: message.to_string(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    NotAuthorized(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    LengthRequired(String),

    #[error("{0}")]
    PayloadTooLarge(String),

    /// The authority could not be established because of a transient store
    /// or reader fault (503, never a denial).
    #[error("{0}")]
    Unavailable(String),

    /// Requires operator action, not a timed retry.
    #[error("{0}")]
    Misconfigured(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("{0}")]
    Internal(String),

    #[error("{error}")]
    SignedFetch {
        status: u16,
        error: String,
        retryable: bool,
    },

    /// A 429, with `Retry-After` when the wait is known.
    #[error("{message}")]
    TooManyRequests {
        message: String,
        retry_after_secs: Option<u64>,
    },
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    pub fn not_authorized(msg: impl Into<String>) -> Self {
        Self::NotAuthorized(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    pub fn unavailable(msg: impl Into<String>) -> Self {
        Self::Unavailable(msg.into())
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::NotAuthorized(_) => 401,
            Self::NotFound(_) => 404,
            Self::LengthRequired(_) => 411,
            Self::PayloadTooLarge(_) => 413,
            Self::Unavailable(_) | Self::Misconfigured(_) => 503,
            Self::Database(_) | Self::Internal(_) => 500,
            Self::SignedFetch { status, .. } => *status,
            Self::TooManyRequests { .. } => 429,
        }
    }
}

/// A permission refusal keeps its own status: 403 was the old route's 401
/// "not authorized to perform operations on this world" (the route maps
/// that arm itself), 503 for "could not tell" and "not configured".
impl From<AuthorityNotEstablished> for ApiError {
    fn from(why: AuthorityNotEstablished) -> Self {
        match &why {
            AuthorityNotEstablished::UndeterminedStoreUnavailable {
                store,
                reason_for_operators,
            } => {
                // Upstream answers 401 here; dcl-one fails closed with a retryable 503 on purpose.
                tracing::error!(store, reason = %reason_for_operators, "world storage: authority undetermined");
                ApiError::Unavailable(
                    "Unauthorized: Failed to verify world permissions".to_string(),
                )
            }
            AuthorityNotEstablished::CredentialNotConfigured {
                environment_variable,
            } => {
                tracing::error!(
                    variable = environment_variable,
                    "world storage: credential not configured"
                );
                ApiError::Misconfigured(why.to_string())
            }
            _ => ApiError::NotAuthorized(
                "Unauthorized: Signer is not authorized to perform operations on this world"
                    .to_string(),
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = match &self {
            ApiError::SignedFetch { error, .. } => ErrorBody::labeled(error, SIGNED_FETCH_MESSAGE),
            ApiError::BadRequest(m) => ErrorBody::labeled("Bad request", m),
            ApiError::NotAuthorized(m) => ErrorBody::labeled("Not Authorized", m),
            ApiError::NotFound(m) => ErrorBody::labeled("Not Found", m),
            ApiError::LengthRequired(m) => ErrorBody::labeled("Length Required", m),
            ApiError::PayloadTooLarge(m) => ErrorBody::labeled("Payload Too Large", m),
            ApiError::Unavailable(m) | ApiError::Misconfigured(m) => {
                ErrorBody::labeled("Service Unavailable", m)
            }
            ApiError::Database(e) => {
                tracing::error!(error = %e, "world storage: sqlx error");
                ErrorBody::new("Internal Server Error")
            }
            ApiError::Internal(m) => {
                tracing::error!(error = %m, "world storage: internal error");
                ErrorBody::new("Internal Server Error")
            }
            ApiError::TooManyRequests {
                message,
                retry_after_secs,
            } => {
                let mut resp = (
                    status,
                    Json(ErrorBody::labeled("Too Many Requests", message)),
                )
                    .into_response();
                if let Some(secs) = retry_after_secs {
                    resp.headers_mut()
                        .insert(axum::http::header::RETRY_AFTER, (*secs).into());
                }
                return resp;
            }
        };
        if matches!(
            self,
            ApiError::Unavailable(_)
                | ApiError::SignedFetch {
                    retryable: true,
                    ..
                }
        ) {
            return (status, [(RETRY_AFTER, "1")], Json(body)).into_response();
        }
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn body_of(resp: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = ApiError::not_found("value not found").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            body_of(resp).await,
            json!({ "ok": false, "error": "Not Found", "message": "value not found" })
        );
    }

    #[tokio::test]
    async fn a_signed_fetch_refusal_carries_the_raw_error_and_the_adr44_sentence() {
        let resp = ApiError::SignedFetch {
            status: 401,
            error: "Invalid signature: x".into(),
            retryable: false,
        }
        .into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            body_of(resp).await,
            json!({ "ok": false, "error": "Invalid signature: x", "message": SIGNED_FETCH_MESSAGE })
        );
    }

    #[tokio::test]
    async fn an_internal_error_never_leaks_its_detail() {
        let resp = ApiError::internal("pool secret detail").into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = body_of(resp).await;
        assert_eq!(body["error"], "Internal Server Error");
        assert!(!body.to_string().contains("secret"));
    }

    #[test]
    fn a_refusal_keeps_the_old_401_and_a_fault_is_503() {
        let refused: ApiError = AuthorityNotEstablished::RefusedLacksAuthority {
            detail: "no".into(),
        }
        .into();
        assert_eq!(refused.status(), 401);
        let down: ApiError =
            AuthorityNotEstablished::store_unavailable("worlds.worlds", &sqlx::Error::PoolTimedOut)
                .into();
        assert_eq!(down.status(), 503);
        let unset: ApiError = AuthorityNotEstablished::CredentialNotConfigured {
            environment_variable: "ENCRYPTION_KEY",
        }
        .into();
        assert_eq!(unset.status(), 503);
    }

    #[test]
    fn only_transient_unavailability_carries_retry_after() {
        let down = ApiError::Unavailable("x".into()).into_response();
        assert_eq!(down.headers()[RETRY_AFTER], "1");
        for permanent in [
            ApiError::from(AuthorityNotEstablished::CredentialNotConfigured {
                environment_variable: "ENCRYPTION_KEY",
            }),
            ApiError::SignedFetch {
                status: 503,
                error: "EIP-1654 not configured".into(),
                retryable: false,
            },
        ] {
            let response = permanent.into_response();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert!(!response.headers().contains_key(RETRY_AFTER));
        }
        let refused = ApiError::not_authorized("no").into_response();
        assert!(!refused.headers().contains_key(RETRY_AFTER));
    }
}
