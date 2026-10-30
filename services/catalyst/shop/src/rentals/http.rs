//! The signatures-server envelopes: `{"ok": true, "data": ...}` on success,
//! `{"ok": false, "error": msg, "message": msg}` on refusal (the shared
//! [`crate::http::api_error`] with the old service's two 500 texts); a
//! database error never publishes its detail.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::json;

use crate::http::api_error::Surface;

pub struct Ok2<T: Serialize>(pub StatusCode, pub T);

impl<T: Serialize> IntoResponse for Ok2<T> {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "ok": true, "data": self.1 }))).into_response()
    }
}

pub struct Signatures;

impl Surface for Signatures {
    const ROUTE: &'static str = "a signatures route";
    const DATABASE_MESSAGE: &'static str = "Server error";
    const INTERNAL_MESSAGE: &'static str = "internal error";
}

pub type ApiError = crate::http::api_error::ApiError<Signatures>;

/// The signatures-server page: zero-based `page`, `pages` rounded up.
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub results: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub pages: i64,
    pub limit: i64,
}

impl<T> PaginatedResponse<T> {
    pub fn new(results: Vec<T>, total: i64, limit: i64, offset: i64) -> Self {
        let page = if limit > 0 { offset / limit } else { 0 };
        let pages = if limit > 0 {
            (total + limit - 1) / limit
        } else {
            0
        };
        Self {
            results,
            total,
            page,
            pages,
            limit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn success_envelope_wire_shape() {
        let data = json!({ "a": 1 });
        let resp = Ok2(StatusCode::OK, data.clone()).into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let legacy = serde_json::to_vec(&json!({ "ok": true, "data": data })).unwrap();
        assert_eq!(bytes.as_ref(), legacy.as_slice());
    }

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = ApiError::not_found("Rental not found").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "error": "Rental not found", "message": "Rental not found" })
        );
    }

    #[tokio::test]
    async fn internal_detail_is_not_published() {
        let resp = ApiError::internal("signer key load failed: /etc/keys/x").into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "error": "internal error", "message": "internal error" })
        );
    }

    #[tokio::test]
    async fn database_detail_is_not_published() {
        let resp = ApiError::from(sqlx::Error::RowNotFound).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "error": "Server error", "message": "Server error" })
        );
    }

    #[test]
    fn pages_round_up_and_page_is_zero_based() {
        let page = PaginatedResponse::new(vec![1, 2], 25, 10, 20);
        assert_eq!(
            (page.page, page.pages, page.limit, page.total),
            (2, 3, 10, 25)
        );
        let empty: PaginatedResponse<i32> = PaginatedResponse::new(vec![], 0, 100, 0);
        assert_eq!((empty.page, empty.pages), (0, 0));
    }
}
