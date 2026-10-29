//! The builder's wire envelopes: errors render as the builder-server
//! `{"ok":false,"error":msg,"message":msg}` body (no `code`, unlike the
//! explore envelope) through the shared [`dcl_http::api_error`], with the
//! old service's "Internal Server Error" for both 500 cases; successes as the
//! shared `{"ok":true,"data":...}` ([`ApiData`], the one the explore routes
//! serialize). A database or internal failure never leaks its detail.

pub use crate::shared::errors::ApiData;
use dcl_http::api_error::Surface;

pub struct Builder;

impl Surface for Builder {
    const ROUTE: &'static str = "a builder route";
    const DATABASE_MESSAGE: &'static str = "Internal Server Error";
    const INTERNAL_MESSAGE: &'static str = "Internal Server Error";
}

pub type ApiError = dcl_http::api_error::ApiError<Builder>;

pub async fn private_response(mut response: axum::response::Response) -> axum::response::Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use serde_json::json;

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = ApiError::not_found("Not found").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "error": "Not found", "message": "Not found" })
        );
    }

    #[tokio::test]
    async fn database_and_internal_errors_do_not_expose_details() {
        for err in [
            ApiError::Database(sqlx::Error::PoolClosed),
            ApiError::internal("secret detail"),
        ] {
            let resp = err.into_response();
            assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
            let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
            let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                v,
                json!({ "ok": false, "error": "Internal Server Error", "message": "Internal Server Error" })
            );
        }
    }
}
