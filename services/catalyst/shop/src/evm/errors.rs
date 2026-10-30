//! The transactions-server error envelope: `{"ok": false, "error": <code>,
//! "message": <text>}` with the upstream's status per code. A database error
//! is logged and answered as `Unknown error`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

pub mod code {
    pub const UNKNOWN: &str = "unknown";
    pub const INVALID_TRANSACTION: &str = "invalid_transaction";
    pub const INVALID_SCHEMA: &str = "invalid_schema";
    pub const INVALID_CONTRACT_ADDRESS: &str = "invalid_contract_address";
    pub const SALE_PRICE_TOO_LOW: &str = "sale_price_too_low";
    pub const QUOTA_REACHED: &str = "quota_reached";
    pub const HIGH_CONGESTION: &str = "high_congestion";
    pub const CONFLICT: &str = "conflict";
    pub const PAYMENT_RULE: &str = crate::guards::ERROR_CODE;
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    InvalidSchema(String),
    #[error("{0}")]
    InvalidSalePrice(String),
    #[error("{0}")]
    InvalidContractAddress(String),
    #[error("{0}")]
    InvalidTransaction(String),
    #[error("{0}")]
    QuotaReached(String),
    #[error("{0}")]
    HighCongestion(String),
    #[error("{0}")]
    RelayReverted(String),
    #[error("{0}")]
    RelayerFailed(String),
    #[error("{0}")]
    RelayerUnavailable(String),
    #[error("{0}")]
    RelayerTimeout(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    MissingTransactionData(String),
    #[error("{0}")]
    MalformedBody(String),
    #[error("{0}")]
    NotImplemented(String),
    #[error("{0}")]
    Conflict(String),
    /// The Shop payment rule refused the operation (`shop::guards::Refused`,
    /// its own envelope: 422, or 503 naming the missing source).
    #[error("{0}")]
    PaymentRule(#[from] crate::guards::Refused),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Internal(String),
}

impl ApiError {
    fn parts(&self) -> (u16, Option<&'static str>) {
        match self {
            ApiError::InvalidSchema(_) => (400, Some(code::INVALID_SCHEMA)),
            ApiError::InvalidSalePrice(_) => (400, Some(code::SALE_PRICE_TOO_LOW)),
            ApiError::InvalidContractAddress(_) => (400, Some(code::INVALID_CONTRACT_ADDRESS)),
            ApiError::InvalidTransaction(_) => (400, Some(code::INVALID_TRANSACTION)),
            ApiError::RelayReverted(_) => (400, Some(code::INVALID_TRANSACTION)),
            ApiError::QuotaReached(_) => (429, Some(code::QUOTA_REACHED)),
            ApiError::HighCongestion(_) => (503, Some(code::HIGH_CONGESTION)),
            ApiError::RelayerUnavailable(_) => (503, Some(code::UNKNOWN)),
            ApiError::RelayerTimeout(_) => (504, Some(code::UNKNOWN)),
            ApiError::Forbidden(_) => (403, Some(code::UNKNOWN)),
            ApiError::NotFound(_) => (404, Some(code::UNKNOWN)),
            ApiError::MissingTransactionData(_) => (400, None),
            ApiError::MalformedBody(_) => (400, None),
            ApiError::NotImplemented(_) => (501, Some(code::UNKNOWN)),
            ApiError::Conflict(_) => (409, Some(code::CONFLICT)),
            ApiError::PaymentRule(refused) => (refused.status().as_u16(), Some(code::PAYMENT_RULE)),
            ApiError::Database(_) => (500, Some(code::UNKNOWN)),
            ApiError::RelayerFailed(_) => (500, Some(code::UNKNOWN)),
            ApiError::Internal(_) => (500, Some(code::UNKNOWN)),
        }
    }

    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.parts().0).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let ApiError::PaymentRule(refused) = self {
            return refused.into_response();
        }
        let (_, err_code) = self.parts();
        let message = match &self {
            ApiError::Database(e) => {
                tracing::error!(error = %e, "economy: sqlx error");
                "Unknown error".to_string()
            }
            other => other.to_string(),
        };
        // ApiErrorBody::new repeats the message as the code (the old shape).
        let error = err_code
            .map(str::to_string)
            .unwrap_or_else(|| message.clone());
        (
            self.status(),
            Json(json!({ "ok": false, "error": error, "message": message })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = ApiError::QuotaReached("relay quota reached".to_string()).into_response();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "error": "quota_reached", "message": "relay quota reached" })
        );
    }

    #[tokio::test]
    async fn an_unlabeled_error_repeats_its_message_as_the_code() {
        let resp = ApiError::MalformedBody("bad json".to_string()).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "error": "bad json", "message": "bad json" })
        );
    }
}
