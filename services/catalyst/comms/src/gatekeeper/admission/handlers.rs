//! The admission routes: the scene adapter tokens (`/get-scene-adapter`,
//! `/get-server-scene-adapter`), the room roster (`/scene-participants` and
//! the destinations feed's `Participants`), `/ping` and `/status`.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Address;

use super::scene;
use crate::gatekeeper::auth::{device_identifier, get_request_ip, verify_signed_fetch};
use crate::gatekeeper::errors::{ApiError, unauthorized};
use crate::gatekeeper::livekit::{
    AccessToken, address_from_identity, build_adapter_url, join_grants,
};
use crate::gatekeeper::moderation::store::UpsertPlayerConnection;
use crate::gatekeeper::{CommsState, SharedState};
use crate::rooms::worlds_service_world_room;
use crate::shared::signed_fetch::SignedPath;
use crate::world_identity::WorldName;
use crate::world_identity::is_world_realm_name;

pub async fn status(State(state): State<SharedState>) -> Json<Value> {
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "currentTime": current_time,
        "commitHash": state.settings.commit_hash.as_deref().unwrap_or(""),
    }))
}

pub async fn ping(OriginalUri(uri): OriginalUri) -> impl IntoResponse {
    uri.path().to_string()
}

#[derive(Debug, Default, Deserialize)]
pub struct SceneAdapterRequest {
    #[serde(rename = "sceneId")]
    pub scene_id: Option<String>,
    pub parcel: Option<String>,
    #[serde(rename = "realmName")]
    pub realm_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SceneAdapterResponse {
    pub adapter: String,
}

fn realm_name_from(metadata: &Value, body: &SceneAdapterRequest) -> Option<String> {
    scene::realm_name_from_metadata(metadata).or_else(|| body.realm_name.clone())
}

fn scene_id_from(metadata: &Value, body: &SceneAdapterRequest) -> Option<String> {
    scene::meta_str(metadata, "sceneId").or_else(|| body.scene_id.clone())
}

async fn record_connection(
    state: &CommsState,
    identity: &Address,
    headers: &HeaderMap,
    device_id: Option<&str>,
) {
    let upsert = UpsertPlayerConnection {
        address: identity.clone(),
        ip_address: get_request_ip(headers),
        device_id: device_id.map(str::to_string),
    };
    if let Err(e) = state.moderation.upsert_connection(upsert).await {
        tracing::warn!(error = %e, address = %identity, "failed to store player connection info");
    }
}

/// POST /get-scene-adapter: a signed fetch (any signer); the realm and scene
/// come from the signed metadata first, the body second; the platform ban
/// (device-widened) then the scene ban refuse before the world access check
/// (`worlds::policy::Policy::can_access`, a local call; a shared-secret
/// world reads the secret from the signed metadata's `secret` field, the
/// scene host's contract); the token is a microphone-only join of the
/// scene's room. The connection record is a social write, skipped while the
/// social switch is off or held; the mint is admission and still answers.
pub async fn get_scene_adapter(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    switch: Option<axum::Extension<crate::writes::Switch>>,
    raw_body: Bytes,
) -> Result<Json<SceneAdapterResponse>, ApiError> {
    // upstream verifies in the handler and answers every refusal as one 401
    let sf = verify_signed_fetch(
        &headers,
        "post",
        &SignedPath::at(&uri, "/get-scene-adapter"),
        &[],
        state.eip1654(),
    )
    .await
    .map_err(|_| unauthorized("Access denied, invalid signed-fetch request"))?;
    let body: SceneAdapterRequest = serde_json::from_slice(&raw_body).unwrap_or_default();
    let identity = sf.signer;
    let realm_name = realm_name_from(&sf.metadata, &body)
        .ok_or_else(|| unauthorized("Access denied, invalid signed-fetch request, no realmName"))?;
    let scene_id = scene_id_from(&sf.metadata, &body).ok_or_else(|| {
        ApiError::bad_request("Access denied, invalid signed-fetch request, no sceneId")
    })?;
    let is_world = is_world_realm_name(&realm_name);
    let resolved = scene::resolve_adapter_room(&state, &realm_name, &scene_id).await?;
    let resolved_scene_id = resolved
        .scene_id
        .as_ref()
        .ok_or_else(|| ApiError::internal("scene resolution returned a lobby"))?
        .as_str();
    let device_id = device_identifier(&sf.metadata);
    let secret = scene::meta_str(&sf.metadata, "secret");
    let record = crate::writes::side_refusal(switch).is_none();
    let ((), gate, world_allowed) = tokio::join!(
        async {
            if record {
                record_connection(&state, &identity, &headers, device_id.as_deref()).await;
            }
        },
        state
            .moderation
            .connection_gate(&identity, device_id.as_deref(), &resolved_scene_id),
        async {
            if !is_world {
                return Ok(true);
            }
            scene::has_world_access_permission(
                &state,
                &identity,
                &realm_name,
                Some(&resolved_scene_id),
                secret.as_deref(),
                device_id.as_deref(),
            )
            .await
        },
    );
    gate.into_result()?;
    if !world_allowed? {
        return Err(unauthorized(
            "Access denied, you are not authorized to access this world",
        ));
    }
    let mut grants = join_grants(&resolved.room);
    grants.can_update_own_metadata = true;
    let mut token = AccessToken::new(
        &state.livekit.api_key,
        &state.livekit.api_secret,
        identity.as_str(),
        grants,
    )
    .with_metadata(serde_json::json!({ "isGuest": sf.is_guest }).to_string());
    token.name = sf
        .metadata
        .get("displayName")
        .and_then(Value::as_str)
        .map(str::to_string);
    let token = token
        .to_jwt()
        .map_err(|e| ApiError::internal(format!("livekit token: {e}")))?;
    Ok(Json(SceneAdapterResponse {
        adapter: build_adapter_url(&state.livekit.client_url, &token),
    }))
}

const AUTH_SERVER_IDENTITY: &str = "authoritative-server";

/// POST /get-server-scene-adapter: only the configured authoritative server
/// wallet (a verified signature, never a body field) gets the host token,
/// which publishes and subscribes under the `authoritative-server` identity.
pub async fn get_server_scene_adapter(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<SceneAdapterResponse>, ApiError> {
    let sf = crate::gatekeeper::auth::verify_server_signed_fetch(
        &headers,
        "post",
        &SignedPath::at(&uri, "/get-server-scene-adapter"),
        state.eip1654(),
    )
    .await
    .map_err(|_| unauthorized("Access denied, invalid signed-fetch request"))?;
    let identity = sf.signer;
    match state.settings.authoritative_server_address.as_ref() {
        Some(expected) if identity == *expected => {}
        _ => return Err(unauthorized("Access denied, invalid server public key")),
    }
    let realm_name = scene::realm_name_from_metadata(&sf.metadata)
        .ok_or_else(|| unauthorized("Access denied, invalid signed-fetch request, no realm"))?;
    let scene_id = scene::meta_str(&sf.metadata, "sceneId").ok_or_else(|| {
        ApiError::bad_request("Access denied, invalid signed-fetch request, no sceneId")
    })?;
    let resolved = scene::resolve_adapter_room(&state, &realm_name, &scene_id).await?;
    let token = AccessToken::new(
        &state.livekit.api_key,
        &state.livekit.api_secret,
        AUTH_SERVER_IDENTITY,
        join_grants(&resolved.room),
    )
    .with_metadata(
        serde_json::json!({ "catalyrstAuthoritativeServer": { "wallet": identity.as_str() } })
            .to_string(),
    )
    .to_jwt()
    .map_err(|e| ApiError::internal(format!("livekit token: {e}")))?;
    Ok(Json(SceneAdapterResponse {
        adapter: build_adapter_url(&state.livekit.client_url, &token),
    }))
}

#[derive(Debug, Deserialize)]
pub struct ParticipantsQuery {
    pub pointer: Option<String>,
    pub realm_name: Option<String>,
    pub room: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SceneParticipantsResponse {
    pub ok: bool,
    pub data: SceneParticipantsData,
}

#[derive(Debug, Serialize)]
pub struct SceneParticipantsData {
    pub addresses: Vec<String>,
}

/// GET /scene-participants: the wallets in the room of a pointer/realm, the
/// scene resolved from the active deployment (a world scene by parcel, a
/// Genesis scene by `content.active_pointers`); a LiveKit failure reads as an
/// empty room, as before.
pub async fn list_participants(
    State(state): State<SharedState>,
    Query(q): Query<ParticipantsQuery>,
) -> Result<Json<SceneParticipantsResponse>, ApiError> {
    Ok(Json(SceneParticipantsResponse {
        ok: true,
        data: SceneParticipantsData {
            addresses: participants(&state, q).await?,
        },
    }))
}

/// Shared roster lookup for HTTP, destinations and the presence archive.
pub async fn participants(
    state: &CommsState,
    q: ParticipantsQuery,
) -> Result<Vec<String>, ApiError> {
    let pointer = q.pointer.as_deref().filter(|s| !s.is_empty());
    let mut realm_name = q.realm_name.or(q.room).filter(|s| !s.is_empty());
    if pointer.is_some() && realm_name.is_none() {
        realm_name = Some("main".to_string());
    }
    if pointer.is_none() && realm_name.is_none() {
        return Err(ApiError::bad_request(
            "Either pointer or realm_name must be provided",
        ));
    }
    let realm = realm_name.as_deref().unwrap_or("main");
    let room_name = if is_world_realm_name(realm) {
        match pointer {
            Some(p) => match scene::participant_room(state, realm, p).await? {
                Some(resolved) => resolved.room,
                None => {
                    return Err(ApiError::not_found(format!(
                        "No scene found for world {realm} at pointer: {p}"
                    )));
                }
            },
            None => {
                let world = WorldName::parse(realm)
                    .map_err(|_| ApiError::bad_request("Invalid realm name"))?;
                if world.display() != realm {
                    return Err(ApiError::bad_request("Invalid realm name"));
                }
                worlds_service_world_room(&world)
            }
        }
    } else {
        let Some(p) = pointer else {
            return Err(ApiError::bad_request(
                "Either pointer with realm_name or a world realm_name must be provided",
            ));
        };
        match scene::participant_room(state, realm, p).await? {
            Some(resolved) => resolved.room,
            None => {
                return Err(ApiError::not_found(format!(
                    "No scene found for pointer: {p}"
                )));
            }
        }
    };
    let identities = if state.livekit.configured {
        state
            .room_service()
            .list_participant_identities(&room_name)
            .await
    } else {
        Vec::new()
    };
    let addresses: Vec<String> = identities
        .iter()
        .filter_map(|id| address_from_identity(id))
        .collect();
    Ok(addresses)
}

/// The destinations feed's connected wallets (`explore::places::live`): the
/// same room resolution as `/scene-participants` (a Genesis scene by its
/// pointer in realm `main`, a world by its room), `None` without LiveKit or
/// when no scene stands at the pointer (the old route's 400/404, which the
/// places client read as unknown).
impl crate::explore::places::live::Participants for CommsState {
    fn participants<'a>(
        &'a self,
        world: bool,
        key: &'a str,
    ) -> futures::future::BoxFuture<'a, Option<Vec<String>>> {
        Box::pin(async move {
            if !self.livekit.configured {
                return None;
            }
            participants(
                self,
                ParticipantsQuery {
                    pointer: (!world).then(|| key.to_string()),
                    realm_name: Some(if world { key } else { "main" }.to_string()),
                    room: None,
                },
            )
            .await
            .ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realm_and_scene_prefer_the_signed_metadata_over_the_body() {
        let body = SceneAdapterRequest {
            scene_id: Some("body-scene".into()),
            parcel: None,
            realm_name: Some("body-realm".into()),
        };
        let meta = serde_json::json!({ "realmName": "main", "sceneId": "meta-scene" });
        assert_eq!(realm_name_from(&meta, &body).as_deref(), Some("main"));
        assert_eq!(scene_id_from(&meta, &body).as_deref(), Some("meta-scene"));
        assert_eq!(
            realm_name_from(&serde_json::json!({}), &body).as_deref(),
            Some("body-realm")
        );
        assert_eq!(
            scene_id_from(&serde_json::json!({}), &body).as_deref(),
            Some("body-scene")
        );
    }
}
