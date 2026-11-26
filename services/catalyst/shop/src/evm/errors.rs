//! The transactions-server error envelope uses `ok`, `message` and `code`.
//! The `error` field also serves existing Shop clients. Database errors are
//! logged and answered as `Unknown error`.

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
    /// The relayer's daily budget for free (non-sale) calls is spent.
    pub const RELAY_BUDGET_SPENT: &str = "relay_budget_spent";
    pub const HIGH_CONGESTION: &str = "high_congestion";
    pub const EXPECTATION_FAILED: &str = "expectation_failed";
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
    /// The address used its relays for the window: `quota` is its cap, `retry_at` the estimated
    /// next opening of that address's quota, the first whole second (unix) at which enough of
    /// its counted relays have left the window. An estimate: the budget across addresses and
    /// later requests can still hold the next relay back. `None` when none will open (a cap of
    /// 0), fewer than `quota` remain by the time it is read, or the read failed.
    #[error("{message}")]
    QuotaReached {
        message: String,
        quota: i64,
        retry_at: Option<i64>,
    },
    /// The relayer's budget for free calls across every address is spent: `retry_at` the
    /// estimated next opening of that budget, as in [`ApiError::QuotaReached`] but over every
    /// free call instead of one address's relays.
    #[error("{message}")]
    RelayBudgetSpent {
        message: String,
        retry_at: Option<i64>,
    },
    #[error("{0}")]
    HighCongestion(String),
    #[error("{0}")]
    RelayReverted(String),
    /// OpenZeppelin reports that no transaction was signed.
    #[error("{0}")]
    RelayerRefused(String),
    #[error("{0}")]
    RelayerFailed(String),
    #[error("{0}")]
    RelayerUnavailable(String),
    #[error("{0}")]
    RelayerTimeout(String),
    /// A terminal relayer status for a transaction with a known hash.
    #[error("{message}")]
    BroadcastFailed { message: String, tx_hash: String },
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
            ApiError::RelayReverted(_) | ApiError::RelayerRefused(_) => {
                (400, Some(code::INVALID_TRANSACTION))
            }
            ApiError::QuotaReached { .. } => (429, Some(code::QUOTA_REACHED)),
            ApiError::RelayBudgetSpent { .. } => (429, Some(code::RELAY_BUDGET_SPENT)),
            ApiError::HighCongestion(_) => (503, Some(code::HIGH_CONGESTION)),
            ApiError::RelayerUnavailable(_) => (503, Some(code::UNKNOWN)),
            ApiError::RelayerTimeout(_) => (504, Some(code::UNKNOWN)),
            ApiError::BroadcastFailed { .. } => (500, Some(code::EXPECTATION_FAILED)),
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

    pub fn into_not_sent_response(self) -> Response {
        self.respond(true)
    }

    fn respond(self, not_sent: bool) -> Response {
        if let ApiError::PaymentRule(refused) = self {
            let mut body = refused.body();
            if not_sent {
                body["code"] = json!(code::PAYMENT_RULE);
                body["not_sent"] = json!(true);
            }
            return refused.respond(body);
        }
        let (_, err_code) = self.parts();
        let message = match &self {
            ApiError::Database(e) => {
                tracing::error!(error = %e, "economy: sqlx error");
                "Unknown error".to_string()
            }
            other => other.to_string(),
        };
        // Parse errors repeat the message in `error` for existing callers.
        let error = err_code
            .map(str::to_string)
            .unwrap_or_else(|| message.clone());
        let mut body = json!({ "ok": false, "error": error, "message": message });
        let legacy_code = match &self {
            ApiError::RelayerRefused(_) => Some(code::EXPECTATION_FAILED),
            _ => err_code.or(not_sent.then_some(code::INVALID_SCHEMA)),
        };
        if let Some(code) = legacy_code {
            body["code"] = json!(code);
        }
        if not_sent {
            body["not_sent"] = json!(true);
        }
        let retry_at = match &self {
            ApiError::QuotaReached {
                quota, retry_at, ..
            } => {
                body["quota"] = json!(quota);
                *retry_at
            }
            ApiError::RelayBudgetSpent { retry_at, .. } => *retry_at,
            _ => None,
        };
        if let Some(at) = retry_at {
            body["retryAt"] = json!(at);
        }
        (self.status(), Json(body)).into_response()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        self.respond(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn body_of(err: ApiError) -> (StatusCode, serde_json::Value) {
        let resp = err.into_response();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let (status, v) = body_of(ApiError::QuotaReached {
            message: "relay quota reached".to_string(),
            quota: 30,
            retry_at: Some(1_795_100_000),
        })
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            v,
            json!({ "ok": false, "error": "quota_reached", "code": "quota_reached", "message": "relay quota reached", "quota": 30, "retryAt": 1_795_100_000 })
        );
    }

    /// A quota with no slot to free names its cap and no time. The spent budget across
    /// addresses says when it frees but names no cap (it is no one address's), and a labeled
    /// error that is neither carries the code under both names only.
    #[tokio::test]
    async fn only_a_quota_refusal_says_its_cap_and_when_it_frees() {
        let (_, closed) = body_of(ApiError::QuotaReached {
            message: "closed".to_string(),
            quota: 0,
            retry_at: None,
        })
        .await;
        assert_eq!(closed["quota"], json!(0));
        assert!(closed.get("retryAt").is_none(), "{closed}");
        let (status, spent) = body_of(ApiError::RelayBudgetSpent {
            message: "spent".to_string(),
            retry_at: Some(1_795_100_000),
        })
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            spent,
            json!({ "ok": false, "error": "relay_budget_spent", "code": "relay_budget_spent", "message": "spent", "retryAt": 1_795_100_000 })
        );
        let (_, labeled) = body_of(ApiError::HighCongestion("busy".to_string())).await;
        assert_eq!(
            labeled,
            json!({ "ok": false, "error": "high_congestion", "code": "high_congestion", "message": "busy" })
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
            json!({ "ok": false, "error": "bad json", "message": "bad json" }),
            "no code field for an unlabeled error"
        );
    }

    #[tokio::test]
    async fn a_large_relayer_refusal_keeps_its_error_envelope() {
        let message = "r".repeat(64 * 1024);
        let resp = ApiError::RelayerRefused(message.clone()).into_not_sent_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(resp.into_body(), 128 * 1024)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["ok"], false);
        assert_eq!(body["code"], code::EXPECTATION_FAILED);
        assert_eq!(body["error"], code::INVALID_TRANSACTION);
        assert_eq!(body["message"], message);
        assert_eq!(body["not_sent"], true);
    }
}
