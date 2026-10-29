//! The old service's own `/health` and `/ping`, served under the
//! `/transactions-api` prefix only (the content server owns `/ping` at the
//! root).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::evm::SharedState;

pub async fn health(State(state): State<SharedState>) -> (StatusCode, Json<Value>) {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let body = json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "database": db_ok,
        "relayer": state.settings.can_relay(),
        "relayer_mode": state.settings.relay_mode(),
        "data_writes_enabled": state.settings.data_writes_enabled,
        "usd_pegged_stale_refusals": crate::evm::oracle::stale_refusal_count(),
    });
    (status, Json(body))
}

pub async fn ping() -> &'static str {
    "pong"
}
