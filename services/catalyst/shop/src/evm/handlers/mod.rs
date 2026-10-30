//! The relay's route handlers, one file per old handler module; the broker,
//! escrow, NAME custody and payments handlers of the old crate live in
//! `shop::checkout` and `shop::treasury` and share the helpers below.

pub mod admin;
pub mod contracts;
pub mod health;
pub mod transactions;

use axum::http::HeaderMap;

use super::SharedState;
use super::errors::ApiError;

pub(crate) fn idempotency_key(headers: &HeaderMap, body_key: Option<&str>) -> Option<String> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .or(body_key)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub(crate) fn require_broadcast_enabled(state: &SharedState, what: &str) -> Result<(), ApiError> {
    if state.runtime.relayer_enabled() {
        return Ok(());
    }
    Err(ApiError::RelayerUnavailable(format!(
        "Broadcasting is paused by the operator (relayer toggle is OFF); {what} are not broadcast while paused."
    )))
}
