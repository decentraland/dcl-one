//! The lambdas-style `{"ok":false,"error":msg,"message":msg}` error envelope
//! the builder, signatures, notifications and badges routes answer with: one
//! enum, one `IntoResponse`, one set of constructors. A [`Surface`] names the
//! route family for the log line and carries the two 500 texts each old
//! service published for a database or internal failure, so the wire of each
//! family is unchanged while the code exists once. Detail never leaves: a
//! database or internal error is logged and answered with the fixed text.

use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// One route family's name and its two 500 texts.
pub trait Surface: 'static {
    /// The log line's subject ("a builder route").
    const ROUTE: &'static str;
    /// The published message of a database failure.
    const DATABASE_MESSAGE: &'static str;
    /// The published message of any other internal failure.
    const INTERNAL_MESSAGE: &'static str;
}

pub enum ApiError<S: Surface> {
    Http {
        status: u16,
        message: String,
    },
    Database(sqlx::Error),
    Internal(String),
    /// Never constructed: it carries the surface marker and nothing else. A
    /// match by value omits it (the `Infallible` field); a match through a
    /// reference spells `Self::Surface(_, never) => match *never {}`.
    #[doc(hidden)]
    Surface(PhantomData<S>, Infallible),
}

impl<S: Surface> fmt::Debug for ApiError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http { status, message } => f
                .debug_struct("Http")
                .field("status", status)
                .field("message", message)
                .finish(),
            Self::Database(e) => f.debug_tuple("Database").field(e).finish(),
            Self::Internal(m) => f.debug_tuple("Internal").field(m).finish(),
            Self::Surface(_, never) => match *never {},
        }
    }
}

impl<S: Surface> fmt::Display for ApiError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http { message, .. } => f.write_str(message),
            Self::Database(e) => write!(f, "database error: {e}"),
            Self::Internal(m) => f.write_str(m),
            Self::Surface(_, never) => match *never {},
        }
    }
}

impl<S: Surface> std::error::Error for ApiError<S> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(e) => Some(e),
            _ => None,
        }
    }
}

impl<S: Surface> From<sqlx::Error> for ApiError<S> {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(e)
    }
}

impl<S: Surface> ApiError<S> {
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

    pub fn status(&self) -> u16 {
        match self {
            Self::Http { status, .. } => *status,
            Self::Database(_) | Self::Internal(_) => 500,
            Self::Surface(_, never) => match *never {},
        }
    }

    /// The status and the published message: the same pair the response carries.
    pub fn wire(self) -> (u16, String) {
        match self {
            Self::Http { status, message } => (status, message),
            Self::Database(err) => {
                tracing::error!(error = %err, "sqlx error on {}", S::ROUTE);
                (500, S::DATABASE_MESSAGE.to_string())
            }
            Self::Internal(m) => {
                tracing::error!(error = %m, "internal error on {}", S::ROUTE);
                (500, S::INTERNAL_MESSAGE.to_string())
            }
        }
    }
}

impl<S: Surface> IntoResponse for ApiError<S> {
    fn into_response(self) -> Response {
        let (status, message) = self.wire();
        let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (
            status,
            Json(json!({ "ok": false, "error": message, "message": message })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Probe;
    impl Surface for Probe {
        const ROUTE: &'static str = "a probe route";
        const DATABASE_MESSAGE: &'static str = "db text";
        const INTERNAL_MESSAGE: &'static str = "internal text";
    }
    type ProbeError = ApiError<Probe>;

    async fn body(resp: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn the_envelope_carries_the_message_under_both_keys_and_no_code() {
        let resp = ProbeError::not_found("Not found").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            body(resp).await,
            json!({ "ok": false, "error": "Not found", "message": "Not found" })
        );
        assert_eq!(ProbeError::conflict("x").status(), 409);
        assert_eq!(ProbeError::service_unavailable("x").status(), 503);
        assert_eq!(ProbeError::forbidden("x").to_string(), "x");
        assert_eq!(
            format!("{:?}", ProbeError::http(418, "teapot")),
            r#"Http { status: 418, message: "teapot" }"#
        );
    }

    #[tokio::test]
    async fn the_surface_decides_the_500_texts_and_detail_never_leaves() {
        let resp = ProbeError::from(sqlx::Error::PoolClosed).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            body(resp).await,
            json!({ "ok": false, "error": "db text", "message": "db text" })
        );
        let resp = ProbeError::internal("secret detail: /etc/keys/x").into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            body(resp).await,
            json!({ "ok": false, "error": "internal text", "message": "internal text" })
        );
        assert_eq!(ProbeError::internal("x").status(), 500);
    }

    #[test]
    fn a_match_on_the_three_wire_variants_is_exhaustive() {
        let status = |e: ProbeError| match e {
            ApiError::Http { status, .. } => status,
            ApiError::Database(_) | ApiError::Internal(_) => 500,
        };
        assert_eq!(status(ProbeError::bad_request("x")), 400);
        assert_eq!(status(ProbeError::Internal("x".into())), 500);
    }
}
