//! `/ping`, `/status` and `POST /debugging/reload {secret, name}`.

use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::{SceneKey, SharedState};
use crate::auth::admin_bearer::timing_safe_eq;

pub async fn ping() -> &'static str {
    "/ping"
}

#[derive(Serialize)]
struct StatusResp {
    #[serde(rename = "commitHash")]
    commit_hash: String,
    version: String,
    #[serde(rename = "currentTime")]
    current_time: i64,
    connections: u32,
    #[serde(rename = "loadedScenes")]
    loaded_scenes: Vec<String>,
}

pub async fn status(State(s): State<SharedState>) -> impl IntoResponse {
    let body = Json(StatusResp {
        commit_hash: s.settings.commit_hash.clone().unwrap_or_default(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        current_time: Utc::now().timestamp_millis(),
        connections: s.scenes.connections(),
        loaded_scenes: s.scenes.loaded(),
    });
    ([(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")], body)
}

#[derive(Deserialize)]
struct ReloadReq {
    secret: String,
    name: String,
}

pub async fn reload(State(s): State<SharedState>, body: axum::body::Bytes) -> impl IntoResponse {
    let Some(expected) = s.settings.debugging_secret.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "SCENE_HOST_DEBUGGING_SECRET not configured",
        )
            .into_response();
    };
    let req: ReloadReq = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("bad json: {e}")).into_response(),
    };
    if !timing_safe_eq(req.secret.as_bytes(), expected.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, "Not authorized").into_response();
    }
    if req.name.is_empty() {
        return (StatusCode::BAD_REQUEST, "Missing scene name").into_response();
    }
    let key = match SceneKey::parse(&req.name) {
        Ok(key) => key,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };
    match s.load(&key).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}
