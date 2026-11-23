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
use crate::gatekeeper::auth::{device_identifier, get_request_ip, verify_wallet_signed_fetch};
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

#[derive(Debug, Serialize)]
pub struct SceneAdapterResponse {
    pub adapter: String,
}

/// The room a scene token joins comes from the signed metadata only: the
/// signed `realmName` (as upstream's `oldValidate`) and `sceneId`.
fn signed_scene_target(metadata: &Value, node_realm: &str) -> Result<(String, String), ApiError> {
    let realm_name = scene::meta_str(metadata, "realmName")
        .ok_or_else(|| unauthorized("Access denied, invalid signed-fetch request, no realmName"))?;
    let realm_name = crate::world_identity::signed_realm_name(
        &realm_name,
        scene::signed_realm_url(metadata),
        node_realm,
    );
    let scene_id = scene::meta_str(metadata, "sceneId").ok_or_else(|| {
        ApiError::bad_request("Access denied, invalid signed-fetch request, no sceneId")
    })?;
    Ok((realm_name, scene_id))
}

/// The `.dcl.eth` world a bare signed realm name can stand for
/// ([`bare_world`]): the Unity explorer signs that name as realm and as
/// scene, with no realm URL. It signs this node's Genesis City realm
/// (`node_realm`) the same way: that name, and a realm name beside a scene
/// id, keep their shared room.
fn stripped_world(realm_name: &str, scene_id: &str, node_realm: &str) -> Option<WorldName> {
    if realm_name != scene_id || realm_name.eq_ignore_ascii_case(node_realm) {
        return None;
    }
    crate::world_identity::bare_world(realm_name, node_realm)
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

/// POST /get-scene-adapter: the wallet's own signed fetch (the scene signer is
/// refused); the realm and scene come from the signed metadata only, the body
/// is ignored, a world's bare name reads as that world ([`stripped_world`]); a
/// Genesis scene id must be a scene active here and not denylisted (else 404; a
/// local preview passes with `ALLOW_LOCAL_PREVIEW`); the platform ban
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
    _body: Bytes,
) -> Result<Json<SceneAdapterResponse>, ApiError> {
    // upstream verifies in the handler and answers every refusal as one 401
    let sf = verify_wallet_signed_fetch(
        &headers,
        "post",
        &SignedPath::at(&uri, "/get-scene-adapter"),
        state.eip1654(),
    )
    .await
    .map_err(|_| unauthorized("Access denied, invalid signed-fetch request"))?;
    let identity = sf.signer;
    if state.deny_list.is_denied(identity.as_str()).await {
        return Err(unauthorized(super::world_mint::DENY_LISTED));
    }
    let (mut realm_name, mut scene_id) =
        signed_scene_target(&sf.metadata, &state.settings.realm_name)?;
    if let Some(world) = stripped_world(&realm_name, &scene_id, &state.settings.realm_name)
        && let Some(active) = state
            .scenes
            .world_active_scene(&world)
            .await
            .map_err(|error| ApiError::internal(error.to_string()))?
        && active.local
    {
        realm_name = world.display().to_string();
        scene_id = active.scene_id.into_string();
    }
    let is_world = is_world_realm_name(&realm_name);
    let resolved = scene::resolve_adapter_room(&state, &realm_name, &scene_id).await?;
    let genesis = if is_world {
        None
    } else {
        scene::require_genesis_deployment(&state, &resolved).await?
    };
    let resolved_scene_id = resolved
        .scene_id
        .as_ref()
        .ok_or_else(|| ApiError::internal("scene resolution returned a lobby"))?
        .as_str();
    // A Genesis scene's bans are on the place of the deployment found.
    let ban_key = genesis.as_ref().unwrap_or(&resolved).ban_key();
    let ban_key = ban_key
        .as_ref()
        .map_or(resolved_scene_id, |key| key.as_str());
    let device_id = device_identifier(&sf.metadata);
    let secret = scene::meta_str(&sf.metadata, "secret");
    let started = if let Some(world) = resolved.realm.world() {
        Some(
            super::world_mint::authorization_started(&state.pool, world)
                .await
                .map_err(|error| {
                    tracing::error!(%error, "world authorization unavailable");
                    ApiError::http(503, "World authorization unavailable")
                })?,
        )
    } else {
        None
    };
    let caller_ip = crate::shared::client_ip::client_ip(&headers);
    let caller = if caller_ip.is_empty() {
        identity.as_str()
    } else {
        &caller_ip
    };
    let record = crate::writes::side_writes_on(switch);
    let (gate, world_allowed) = tokio::join!(
        async {
            // Record only an admitted connection: a refused one must not replace the recorded device.
            let gate = state
                .moderation
                .connection_gate(&identity, device_id.as_deref(), ban_key)
                .await;
            if record && gate.is_allowed() {
                record_connection(&state, &identity, &headers, device_id.as_deref()).await;
            }
            gate
        },
        async {
            if !is_world {
                return Ok(scene::WorldAccess::Allowed);
            }
            scene::world_access(
                &state,
                &identity,
                &realm_name,
                Some(resolved_scene_id),
                secret.as_deref(),
                device_id.as_deref(),
                caller,
            )
            .await
        },
    );
    gate.into_result()?;
    match world_allowed? {
        scene::WorldAccess::Allowed => {}
        // A ban of the scene (under its own key or a successor's) is
        // answered apart from the world's refusal, as upstream.
        scene::WorldAccess::SceneBanned => {
            return Err(crate::gatekeeper::errors::forbidden(
                crate::gatekeeper::moderation::store::SCENE_BANNED_MSG,
            ));
        }
        scene::WorldAccess::Refused => {
            return Err(unauthorized(
                "Access denied, you are not authorized to access this world",
            ));
        }
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
    if let (Some(world), Some(started)) = (resolved.realm.world(), started) {
        let proof = super::world_mint::authorization_proof(
            &state.livekit.api_secret,
            world,
            identity.as_str(),
            &resolved.room,
            started,
        );
        token = token.with_attribute(
            super::world_mint::AUTHORIZATION_ATTRIBUTE,
            serde_json::json!({super::world_mint::AUTHORIZATION_FIELD: proof}).to_string(),
        );
    }
    token.name = sf
        .metadata
        .get("displayName")
        .and_then(Value::as_str)
        .map(str::to_string);
    let token = token
        .to_jwt()
        .map_err(|e| ApiError::internal(format!("livekit token: {e}")))?;
    // A scene admin joining a room that is casting becomes a presenter; off
    // the response, bounded, and never a reason to refuse the join (upstream
    // logs it).
    crate::gatekeeper::streaming::cast::spawn_admin_presenter_on_join(
        state.clone(),
        resolved.room.clone(),
        identity.clone(),
    );
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
    let realm_name = scene::realm_name_from_metadata(&sf.metadata, &state.settings.realm_name)
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

impl CommsState {
    /// The destinations feed's connected wallets (`serve` hands them to
    /// Explore's live counts): the same room resolution as
    /// `/scene-participants` (a Genesis scene by its pointer in realm `main`,
    /// a world by its room), `None` without LiveKit or when no scene stands at
    /// the pointer (the old route's 400/404, which the places client read as
    /// unknown).
    pub async fn live_participants(&self, world: bool, key: &str) -> Option<Vec<String>> {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_comes_from_signed_realm_name_and_scene_id_only() {
        let meta = serde_json::json!({ "realmName": "main", "sceneId": "meta-scene" });
        let (realm, scene) = signed_scene_target(&meta, "main").unwrap();
        assert_eq!((realm.as_str(), scene.as_str()), ("main", "meta-scene"));

        let server_name_only =
            serde_json::json!({ "realm": { "serverName": "main" }, "sceneId": "s" });
        assert_eq!(
            signed_scene_target(&server_name_only, "main")
                .unwrap_err()
                .code,
            401
        );
        let no_scene = serde_json::json!({ "realmName": "main" });
        assert_eq!(
            signed_scene_target(&no_scene, "main").unwrap_err().code,
            400
        );
    }

    #[test]
    fn a_bare_name_signed_as_realm_and_scene_can_name_a_world() {
        let world =
            |realm, scene| stripped_world(realm, scene, "node").map(|w| w.display().to_string());
        assert_eq!(world("Label", "Label").as_deref(), Some("Label.dcl.eth"));
        for (realm, scene) in [
            ("main", "bafkreiscene"),
            ("Node", "Node"),
            ("LocalPreview", "LocalPreview"),
            ("Main", "Main"),
            ("a.dcl.eth", "a.dcl.eth"),
            (" a", " a"),
        ] {
            assert_eq!(world(realm, scene), None, "{realm}");
        }
    }
}
