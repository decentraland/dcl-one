//! Storage policy shared by native persistence and the delivery queue.
//! Keep the browser adapter's policy in parity (covered by its storage tests).
pub const MAX_AGE_MS: u64 = 24 * 60 * 60 * 1000;
pub const FUTURE_TOLERANCE_MS: u64 = 5 * 60 * 1000;
pub const TOMBSTONE_MS: u64 = MAX_AGE_MS + FUTURE_TOLERANCE_MS;
pub const MAX_PENDING: usize = 32;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn valid_event(event: &serde_json::Value) -> bool {
    ["login_id", "wallet", "endpoint"]
        .iter()
        .all(|key| event[*key].as_str().is_some_and(|value| !value.is_empty()))
        && event["occurred_at"]
            .as_u64()
            .is_some_and(|at| at <= 9_007_199_254_740_991)
}
