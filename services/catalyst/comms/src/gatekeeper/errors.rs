//! The comms-gatekeeper error envelope: `{"error": msg}` or, for a labelled
//! refusal, `{"error": label, "message": msg}`; an internal fault never leaks
//! its detail. A permission refusal from `social::identity` maps onto it with
//! the status the refusal carries (401/403/503), never a flattened 403.
//!
//! A signed-fetch refusal on a route upstream guards with the crypto
//! middleware (`scene-admin`, `scene-bans`, `scene-stream-access`,
//! `private-messages/token`) is the middleware's own `{"ok": false,
//! "message": msg}`, a 5xx of which says only "Internal error"; the two
//! adapter routes verify in their handler and answer the handler envelope.

use axum::Json;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header::RETRY_AFTER};
use axum::response::{IntoResponse, Response};

use crate::AuthorityNotEstablished;

#[derive(serde::Serialize)]
struct ErrorBody {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(serde::Serialize)]
struct MiddlewareBody {
    ok: bool,
    message: String,
}

#[derive(Debug)]
pub struct ApiError {
    pub code: u16,
    pub message: String,
    pub error_label: Option<String>,
    pub is_internal: bool,
    pub is_middleware: bool,
}

impl ApiError {
    pub fn http(code: u16, message: impl Into<String>) -> Self {
        ApiError {
            code,
            message: message.into(),
            error_label: None,
            is_internal: false,
            is_middleware: false,
        }
    }

    pub fn labeled(code: u16, label: impl Into<String>, message: impl Into<String>) -> Self {
        ApiError {
            code,
            message: message.into(),
            error_label: Some(label.into()),
            is_internal: false,
            is_middleware: false,
        }
    }

    /// The crypto middleware's refusal, `{"ok": false, "message": msg}`.
    pub fn middleware_refusal(code: u16, message: impl Into<String>) -> Self {
        ApiError {
            code,
            message: message.into(),
            error_label: None,
            is_internal: false,
            is_middleware: true,
        }
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        ApiError::http(400, msg)
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        ApiError::http(404, msg)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        ApiError {
            code: 500,
            message: msg.into(),
            error_label: None,
            is_internal: true,
            is_middleware: false,
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "sqlx error");
        ApiError::internal("Internal Server Error")
    }
}

/// The refusal's own status: 401 for a missing/invalid credential, 403 for
/// "identified and found wanting", 503 for "could not tell" or "not
/// configured" (the operator text stays in the log, not on the wire).
impl From<AuthorityNotEstablished> for ApiError {
    fn from(why: AuthorityNotEstablished) -> Self {
        let status = why.http_status();
        let message = match &why {
            AuthorityNotEstablished::RefusedLacksAuthority { detail } => detail.clone(),
            AuthorityNotEstablished::AuthenticationMissingOrInvalid { detail } => {
                (*detail).to_string()
            }
            AuthorityNotEstablished::PresentedSharedSecretDidNotMatch => why.to_string(),
            AuthorityNotEstablished::UndeterminedStoreUnavailable {
                store,
                reason_for_operators,
            } => {
                tracing::error!(store, reason = %reason_for_operators, "authority undetermined: backing store unavailable");
                format!("could not determine authority: backing store {store} unavailable")
            }
            AuthorityNotEstablished::CredentialNotConfigured {
                environment_variable,
            } => {
                tracing::error!(
                    variable = environment_variable,
                    "authority undetermined: credential not configured"
                );
                why.to_string()
            }
        };
        ApiError::http(status, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut headers = HeaderMap::new();
        if status == StatusCode::SERVICE_UNAVAILABLE {
            headers.insert(RETRY_AFTER, HeaderValue::from_static("1"));
        }
        if self.is_middleware {
            let message = if self.code >= 500 {
                "Internal error".to_string()
            } else {
                self.message
            };
            let body = MiddlewareBody { ok: false, message };
            return (status, headers, Json(body)).into_response();
        }
        let body = if self.is_internal {
            ErrorBody {
                error: "Internal Server Error".to_string(),
                message: None,
            }
        } else if let Some(label) = self.error_label {
            ErrorBody {
                error: label,
                message: Some(self.message),
            }
        } else {
            ErrorBody {
                error: self.message,
                message: None,
            }
        };
        (status, headers, Json(body)).into_response()
    }
}

pub fn auth_error(status: u16, msg: impl Into<String>) -> ApiError {
    ApiError::http(status, msg)
}

pub fn forbidden(msg: impl Into<String>) -> ApiError {
    ApiError::http(403, msg)
}

pub fn unauthorized(msg: impl Into<String>) -> ApiError {
    ApiError::http(401, msg)
}

pub fn conflict(msg: impl Into<String>) -> ApiError {
    ApiError::labeled(409, "Conflict", msg)
}

pub fn not_found_labeled(msg: impl Into<String>) -> ApiError {
    ApiError::labeled(404, "Not Found", msg)
}

pub fn service_unavailable(msg: impl Into<String>) -> ApiError {
    ApiError::http(503, msg)
}

/// Percent-encodes everything but the unreserved set, so a world name or a
/// permission name can never add a path segment to an upstream URL.
pub fn encode_path_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn undetermined_token_responses_preserve_envelopes_and_request_retry() {
        let authority = AuthorityNotEstablished::store_unavailable(
            "social.scene_bans",
            &sqlx::Error::PoolTimedOut,
        );
        for (error, expected) in [
            (
                ApiError::from(authority),
                json!({"error": "could not determine authority: backing store social.scene_bans unavailable"}),
            ),
            (
                ApiError::middleware_refusal(503, "operator detail"),
                json!({"ok": false, "message": "Internal error"}),
            ),
        ] {
            let response = error.into_response();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers()[RETRY_AFTER], "1");
            let bytes = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
                expected
            );
        }
        assert!(
            !forbidden("denied")
                .into_response()
                .headers()
                .contains_key(RETRY_AFTER)
        );
    }

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = conflict("scene already banned").into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "error": "Conflict", "message": "scene already banned" })
        );
    }

    #[tokio::test]
    async fn an_internal_error_never_leaks_its_detail() {
        let resp = ApiError::internal("livekit token: secret detail").into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v, json!({ "error": "Internal Server Error" }));
    }

    #[test]
    fn a_refusal_keeps_its_own_status() {
        let refused: ApiError = AuthorityNotEstablished::RefusedLacksAuthority {
            detail: "not an admin".into(),
        }
        .into();
        assert_eq!(
            (refused.code, refused.message.as_str()),
            (403, "not an admin")
        );
        let down: ApiError = AuthorityNotEstablished::store_unavailable(
            "social.scene_admins",
            &sqlx::Error::PoolTimedOut,
        )
        .into();
        assert_eq!(down.code, 503);
        let unset: ApiError = AuthorityNotEstablished::CredentialNotConfigured {
            environment_variable: "COMMS_GATEKEEPER_AUTH_TOKEN",
        }
        .into();
        assert_eq!(unset.code, 503);
        let none: ApiError = AuthorityNotEstablished::AuthenticationMissingOrInvalid {
            detail: "no bearer",
        }
        .into();
        assert_eq!(none.code, 401);
    }

    #[test]
    fn encode_path_segment_neutralizes_url_metacharacters() {
        assert_eq!(encode_path_segment("myworld.dcl.eth"), "myworld.dcl.eth");
        assert_eq!(encode_path_segment("a/b"), "a%2Fb");
        assert_eq!(encode_path_segment("a?x=1"), "a%3Fx%3D1");
        assert_eq!(encode_path_segment("a#frag"), "a%23frag");
        assert_eq!(encode_path_segment("a%2F"), "a%252F");
        assert_eq!(
            encode_path_segment("evil/../permissions"),
            "evil%2F..%2Fpermissions"
        );
        assert_eq!(
            encode_path_segment("name with space"),
            "name%20with%20space"
        );
    }
}
