//! The `{"ok":false,"error":msg,"message":msg}` envelope the badges service
//! answered with (the lambdas-style `ApiErrorBody`, distinct from the content
//! server's `{"error"}`): the shared [`crate::http::api_error`] with the two
//! old 500 texts. The notifications store, mailer and admin gate raise it
//! too, and the inbox routes answer it as [`NotificationsError`]: upstream's
//! inbox renders every refusal with `@dcl/http-commons`, `{"error": <class>,
//! "message": msg}` by status (a 500 says only `{"error": "Internal Server
//! Error"}`), and its signed-fetch gate as `{"error": <why>, "message":
//! <the ADR-44 sentence>}`, never with an `ok` key (the explorer boot corpus
//! row 40). Internal detail is logged, never published.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::communities::http::ADR44_MESSAGE;
use crate::http::api_error::Surface;

pub struct Notifications;

impl Surface for Notifications {
    const ROUTE: &'static str = "a notifications route";
    const DATABASE_MESSAGE: &'static str = "database error";
    const INTERNAL_MESSAGE: &'static str = "internal error";
}

pub type ApiError = crate::http::api_error::ApiError<Notifications>;

/// An inbox route's refusal on the wire of upstream's `@dcl/http-commons`.
#[derive(Debug)]
pub enum NotificationsError {
    Api(ApiError),
    /// The signed-fetch gate: the verifier's reason under `error`, the
    /// ADR-44 sentence under `message`.
    SignedFetch {
        status: u16,
        error: String,
    },
}

impl From<ApiError> for NotificationsError {
    fn from(e: ApiError) -> Self {
        NotificationsError::Api(e)
    }
}

impl From<sqlx::Error> for NotificationsError {
    fn from(e: sqlx::Error) -> Self {
        NotificationsError::Api(ApiError::from(e))
    }
}

impl IntoResponse for NotificationsError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            NotificationsError::SignedFetch { status, error } => {
                (status, json!({ "error": error, "message": ADR44_MESSAGE }))
            }
            NotificationsError::Api(e) => {
                let (status, message) = e.wire();
                let body = match status {
                    400 => json!({ "error": "Bad request", "message": message }),
                    401 => json!({ "error": "Not Authorized", "message": message }),
                    404 => json!({ "error": "Not Found", "message": message }),
                    500.. => json!({ "error": "Internal Server Error" }),
                    _ => json!({ "error": message, "message": message }),
                };
                (status, body)
            }
        };
        let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::{IntoResponse, Response};
    use serde_json::json;

    async fn body(resp: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = ApiError::unauthorized("signed fetch required").into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            body(resp).await,
            json!({ "ok": false, "error": "signed fetch required", "message": "signed fetch required" })
        );
        let resp = ApiError::not_found("badge not found").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            body(resp).await,
            json!({ "ok": false, "error": "badge not found", "message": "badge not found" })
        );
    }

    #[tokio::test]
    async fn the_inbox_answers_the_http_commons_envelope() {
        let resp = NotificationsError::from(ApiError::bad_request("Invalid email")).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body(resp).await,
            json!({ "error": "Bad request", "message": "Invalid email" })
        );
        let resp =
            NotificationsError::from(ApiError::unauthorized("Turnstile verification failed"))
                .into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            body(resp).await,
            json!({ "error": "Not Authorized", "message": "Turnstile verification failed" })
        );
        let resp = NotificationsError::from(sqlx::Error::RowNotFound).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            body(resp).await,
            json!({ "error": "Internal Server Error" })
        );
        let resp = NotificationsError::SignedFetch {
            status: 400,
            error: "Invalid Auth Chain".into(),
        }
        .into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body(resp).await,
            json!({ "error": "Invalid Auth Chain", "message": ADR44_MESSAGE })
        );
    }

    #[tokio::test]
    async fn internal_detail_is_not_published() {
        let resp =
            ApiError::Internal("sendgrid request failed: bad key".to_string()).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            body(resp).await,
            json!({ "ok": false, "error": "internal error", "message": "internal error" })
        );
        let resp = ApiError::Database(sqlx::Error::RowNotFound).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            body(resp).await,
            json!({ "ok": false, "error": "database error", "message": "database error" })
        );
    }
}
