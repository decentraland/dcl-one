//! `/admin/scene/{scene}/{crdt,kick-all,reset}`: operator inspection of a
//! hosted scene behind the bearer `SCENE_HOST_ADMIN_TOKEN`; an unset token
//! refuses every caller (fail closed, `unconfigured_token_fails_closed`);
//! the parse and compare are the shared [`crate::auth::admin_bearer`] gate.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use super::crdt::{CrdtMessage, decode_batch};
use super::{SceneKey, SharedState};

pub(super) struct Forbidden;

impl IntoResponse for Forbidden {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::FORBIDDEN, "Not authorized").into_response()
    }
}

pub(super) fn authorize(expected: Option<&str>, headers: &HeaderMap) -> Result<(), Forbidden> {
    crate::auth::admin_bearer::check(expected, headers).map_err(|_| Forbidden)
}

fn not_loaded(scene_name: &str) -> axum::response::Response {
    (StatusCode::NOT_FOUND, format!("{scene_name} is not loaded")).into_response()
}

#[derive(Serialize)]
struct KickAllResp {
    scene: String,
    kicked: usize,
}

pub async fn kick_all(
    State(s): State<SharedState>,
    headers: HeaderMap,
    Path(scene_name): Path<String>,
) -> axum::response::Response {
    if let Err(forbidden) = authorize(s.settings.admin_token.as_deref(), &headers) {
        return forbidden.into_response();
    }
    let Some(scene) = SceneKey::parse(&scene_name)
        .ok()
        .and_then(|k| s.scenes.get(&k.room_key()))
    else {
        return not_loaded(&scene_name);
    };
    let kicked = scene.kick_all();
    tracing::info!(scene = %scene_name, kicked, "admin kick-all");
    Json(KickAllResp {
        scene: scene_name,
        kicked,
    })
    .into_response()
}

#[derive(Serialize)]
struct CrdtMsgView {
    #[serde(rename = "type")]
    kind: &'static str,
    entity: u32,
    #[serde(rename = "componentId", skip_serializing_if = "Option::is_none")]
    component_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<u32>,
    #[serde(rename = "dataLen", skip_serializing_if = "Option::is_none")]
    data_len: Option<usize>,
}

#[derive(Serialize)]
struct InspectResp {
    scene: String,
    hash: String,
    connections: usize,
    #[serde(rename = "snapshotBytes")]
    snapshot_bytes: usize,
    #[serde(rename = "messageCount")]
    message_count: usize,

    #[serde(rename = "snapshotHex", skip_serializing_if = "Option::is_none")]
    snapshot_hex: Option<String>,

    messages: Vec<CrdtMsgView>,
}

fn view(msg: &CrdtMessage) -> CrdtMsgView {
    match msg {
        CrdtMessage::Put {
            entity,
            component_id,
            timestamp,
            data,
        } => CrdtMsgView {
            kind: "PUT_COMPONENT",
            entity: *entity,
            component_id: Some(*component_id),
            timestamp: Some(*timestamp),
            data_len: Some(data.len()),
        },
        CrdtMessage::Append {
            entity,
            component_id,
            timestamp,
            data,
        } => CrdtMsgView {
            kind: "APPEND_VALUE",
            entity: *entity,
            component_id: Some(*component_id),
            timestamp: Some(*timestamp),
            data_len: Some(data.len()),
        },
        CrdtMessage::DeleteComponent {
            entity,
            component_id,
            timestamp,
        } => CrdtMsgView {
            kind: "DELETE_COMPONENT",
            entity: *entity,
            component_id: Some(*component_id),
            timestamp: Some(*timestamp),
            data_len: None,
        },
        CrdtMessage::DeleteEntity { entity } => CrdtMsgView {
            kind: "DELETE_ENTITY",
            entity: *entity,
            component_id: None,
            timestamp: None,
            data_len: None,
        },
    }
}

#[derive(Deserialize, Default)]
pub struct InspectQuery {
    /// `snapshotHex=false` leaves the raw hex dump out; it is included by default.
    #[serde(rename = "snapshotHex")]
    snapshot_hex: Option<bool>,
}

pub async fn inspect_crdt(
    State(s): State<SharedState>,
    headers: HeaderMap,
    Path(scene_name): Path<String>,
    Query(q): Query<InspectQuery>,
) -> axum::response::Response {
    if let Err(forbidden) = authorize(s.settings.admin_token.as_deref(), &headers) {
        return forbidden.into_response();
    }
    let Some(scene) = SceneKey::parse(&scene_name)
        .ok()
        .and_then(|k| s.scenes.get(&k.room_key()))
    else {
        return not_loaded(&scene_name);
    };
    let snapshot = scene.snapshot();
    let messages: Vec<CrdtMsgView> = decode_batch(&snapshot).iter().map(view).collect();
    Json(InspectResp {
        scene: scene_name,
        hash: scene.scene_hash(),
        connections: scene.client_count(),
        snapshot_bytes: snapshot.len(),
        message_count: messages.len(),
        snapshot_hex: q
            .snapshot_hex
            .unwrap_or(true)
            .then(|| hex::encode(&snapshot)),
        messages,
    })
    .into_response()
}

#[derive(Serialize)]
struct ResetResp {
    scene: String,
    kicked: usize,
    reloaded: bool,
}

pub async fn reset_state(
    State(s): State<SharedState>,
    headers: HeaderMap,
    Path(scene_name): Path<String>,
) -> axum::response::Response {
    if let Err(forbidden) = authorize(s.settings.admin_token.as_deref(), &headers) {
        return forbidden.into_response();
    }
    let Ok(key) = SceneKey::parse(&scene_name) else {
        return not_loaded(&scene_name);
    };
    let Some(scene) = s.scenes.get(&key.room_key()) else {
        return not_loaded(&scene_name);
    };
    let kicked = scene.kick_all();
    drop(scene);
    match s.load(&key).await {
        Ok(()) => {
            tracing::info!(scene = %scene_name, kicked, "admin reset-state (kick + reload)");
            Json(ResetResp {
                scene: scene_name,
                kicked,
                reloaded: true,
            })
            .into_response()
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("kicked {kicked} but reload failed: {e}"),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bearer(tok: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {tok}").parse().unwrap());
        h
    }

    #[test]
    fn unconfigured_token_fails_closed() {
        assert!(authorize(None, &bearer("anything")).is_err());
    }

    #[test]
    fn wrong_and_missing_bearer_rejected() {
        assert!(authorize(Some("secret"), &HeaderMap::new()).is_err());
        assert!(authorize(Some("secret"), &bearer("nope")).is_err());
    }

    #[test]
    fn correct_bearer_accepted() {
        assert!(authorize(Some("secret"), &bearer("secret")).is_ok());
    }

    #[test]
    fn empty_configured_secret_must_reject_empty_bearer() {
        assert!(
            authorize(Some(""), &bearer("")).is_err(),
            "empty configured admin_token accepted an empty Bearer -- fail-open bypass"
        );
        assert!(authorize(Some(""), &HeaderMap::new()).is_err());
    }

    #[test]
    fn empty_bearer_rejected_when_secret_set() {
        assert!(authorize(Some("secret"), &bearer("")).is_err());
    }
}
