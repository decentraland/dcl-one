use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use super::SharedState;
use super::wire_types::{ChallengeResponse, ContentStatusResponse, SynchronizationStatus};

/// A heartbeat older than this means the sync worker is down.
const HEARTBEAT_MAX_AGE_MS: i64 = 300_000;

pub async fn get_status(State(state): State<SharedState>) -> impl IntoResponse {
    let sync_state = state.sync.state_name().to_string();
    let now_ms = chrono::Utc::now().timestamp_millis();

    let (last_heartbeat, up) = match state.sync.heartbeat_ms() {
        Some(heartbeat) => (
            Some(heartbeat),
            Some(now_ms - heartbeat < HEARTBEAT_MAX_AGE_MS),
        ),
        None => (None, None),
    };

    let body = ContentStatusResponse {
        version: state.settings.version.clone(),
        commit_hash: state
            .settings
            .commit_hash
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        eth_network: state.settings.eth_network.clone(),
        synchronization_status: SynchronizationStatus {
            last_sync_with_dao: now_ms,
            synchronization_state: sync_state.clone(),
            sync_frontier: state.sync.sync_frontier_ms(),
            last_heartbeat,
            up,
            cluster_extra: serde_json::Map::new(),
        },
    };

    let status = if sync_state == "Failed" {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };

    (status, Json(body))
}

/// A fresh challenge text (the old `UuidChallengeSupervisor`); the console's
/// `challenge/refresh` answers one too.
pub fn challenge_text() -> String {
    format!("dcl-crypto-{}", uuid::Uuid::new_v4())
}

/// A fresh challenge per call: the realm proves ownership of its content
/// server by echoing it, and nothing is remembered between calls.
pub async fn get_challenge() -> impl IntoResponse {
    Json(ChallengeResponse {
        challenge_text: challenge_text(),
    })
}
