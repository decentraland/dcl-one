use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use super::SharedState;
use super::wire_types::{ChallengeResponse, ContentStatusResponse, SynchronizationStatus};

/// A heartbeat older than this means the sync worker is down.
const HEARTBEAT_MAX_AGE_MS: i64 = 300_000;

/// Always 200, as upstream at the pinned revision (no component reports a
/// status check there). `lastSyncWithDAO` is the last sync heartbeat, this
/// node's equivalent of upstream's last DAO-list refresh (this node has no
/// DAO list; both are refreshed while the node follows its peers), and 0
/// before the first one, as upstream's `timeOfLastSync` starts.
pub async fn get_status(State(state): State<SharedState>) -> impl IntoResponse {
    let sync_state = state.sync.state_name().to_string();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let heartbeat = state.sync.heartbeat_ms();

    let (last_heartbeat, up) = match heartbeat {
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
            .unwrap_or_else(|| "Unknown".to_string()),
        eth_network: state.settings.eth_network.clone(),
        synchronization_status: SynchronizationStatus {
            last_sync_with_dao: heartbeat.unwrap_or(0),
            synchronization_state: sync_state,
            sync_frontier: state.sync.sync_frontier_ms(),
            last_heartbeat,
            up,
            cluster_extra: serde_json::Map::new(),
        },
    };

    (StatusCode::OK, Json(body))
}

/// A new challenge text: a bare UUID v4, as upstream's `uuidv4()`.
pub fn challenge_text() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// The process's challenge: the realm proves ownership of its content server
/// by echoing it, and it stays the same until the process restarts.
pub async fn get_challenge(State(state): State<SharedState>) -> impl IntoResponse {
    Json(ChallengeResponse {
        challenge_text: state.challenge(),
    })
}
