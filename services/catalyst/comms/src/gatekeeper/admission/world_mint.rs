//! The worlds service's comms mints: `POST /worlds/{name}/comms` (the world
//! lobby), `POST /worlds/{name}/scenes/{id}/comms` (a scene deployed in the
//! world) and `POST /get-comms-adapter/world-{name}` (the lobby by room id),
//! at the root and under `/worlds-content-server`, answering in the worlds
//! envelope (`crate::worlds_envelope`). `worlds::core` assigns them to
//! comms, which owns the LiveKit transport: the room is the worlds service's
//! contract (`rooms`, `world-<name>` / `scene-<name>-<id>`, resolved by
//! `SceneIdentity`), the decision the ONE permission core's
//! (`AccessPolicy::is_banned`, `AccessPolicy::can_access`), the capacity the
//! live presence's (`crate::presence`, `MAX_USERS_PER_WORLD`, 100), and a
//! shared-secret world's wrong secrets are throttled per world and caller (3
//! a minute). The old explorer-api shim proxied the two `/worlds/...` paths
//! here; they are these routes now. The old service's external wallet
//! denylist is checked through the shared DENY_LIST_URL reader, alongside
//! the operator's block list, platform and scene bans.

use std::time::Duration;

use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Uri};
use axum::routing::post;
use axum::{Json, Router};
use serde::Serialize;

use crate::access::{Access, AccessType, Ban};
use crate::gatekeeper::SharedState;
use crate::gatekeeper::livekit::{AccessToken, build_adapter_url, join_grants};
use crate::limits::RateLimiter;
use crate::rooms::{ResolveError, RoomRequest, WORLD_ROOM_PREFIX};
use crate::shared::signed_fetch::SignedPath;
use crate::signed_fetch::{EXPLORER_METADATA_KEYS, map_auth_error_api, require_verified};
use crate::world_identity::{SceneId, WorldName};
use crate::worlds_envelope::ApiError;

pub const RATE_LIMIT_MAX_ATTEMPTS: u32 = 3;
pub const RATE_LIMIT_WINDOW_SECONDS: u64 = 60;
pub const DEFAULT_MAX_USERS_PER_WORLD: i64 = 100;

/// The mints' process state: the shared-secret throttle and the capacity.
pub struct MintState {
    pub(crate) limiter: RateLimiter,
    pub(crate) max_users_per_world: i64,
}

impl Default for MintState {
    fn default() -> Self {
        Self {
            limiter: RateLimiter::new(
                RATE_LIMIT_MAX_ATTEMPTS,
                Duration::from_secs(RATE_LIMIT_WINDOW_SECONDS),
            ),
            max_users_per_world: DEFAULT_MAX_USERS_PER_WORLD,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct CommsAdapterResponse {
    pub fixed_adapter: String,
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/worlds/{world_name}/comms", post(world_comms))
        .route(
            "/worlds/{world_name}/scenes/{scene_id}/comms",
            post(world_scene_comms),
        )
        .route("/get-comms-adapter/{room_id}", post(get_comms_adapter))
        .with_state(state)
}

async fn world_comms(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<CommsAdapterResponse>, ApiError> {
    let path = SignedPath::from_uris(&original, &uri);
    mint(&state, &world_name, None, &path, &headers, false).await
}

async fn get_comms_adapter(
    State(state): State<SharedState>,
    Path(room_id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<CommsAdapterResponse>, ApiError> {
    let Some(world_name) = room_id.strip_prefix(WORLD_ROOM_PREFIX) else {
        return Err(ApiError::bad_request("Invalid room id requested."));
    };
    let path = SignedPath::from_uris(&original, &uri);
    mint(&state, world_name, None, &path, &headers, true).await
}

async fn world_scene_comms(
    State(state): State<SharedState>,
    Path((world_name, scene_id)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<CommsAdapterResponse>, ApiError> {
    let path = SignedPath::from_uris(&original, &uri);
    mint(&state, &world_name, Some(&scene_id), &path, &headers, false).await
}

fn undetermined(e: impl std::fmt::Display) -> ApiError {
    tracing::error!(error = %e, "world comms mint: access could not be decided");
    ApiError::service_unavailable("World access could not be decided; retry shortly.")
}

fn require_explorer_handshake(metadata: &serde_json::Value) -> Result<(), ApiError> {
    if metadata.get("signer").and_then(|v| v.as_str()) != Some("dcl:explorer")
        || metadata.get("intent").and_then(|v| v.as_str()) != Some("dcl:explorer:comms-handshake")
    {
        return Err(ApiError::bad_request("Access denied, invalid metadata"));
    }
    Ok(())
}

async fn mint(
    state: &SharedState,
    world_name: &str,
    scene_id: Option<&str>,
    path: &SignedPath,
    headers: &HeaderMap,
    explorer_handshake: bool,
) -> Result<Json<CommsAdapterResponse>, ApiError> {
    let auth = require_verified(
        headers,
        "post",
        path,
        EXPLORER_METADATA_KEYS,
        state.eip1654(),
    )
    .await
    .map_err(map_auth_error_api)?;
    if explorer_handshake {
        require_explorer_handshake(&auth.metadata)?;
    }
    let identity = auth.signer.clone();
    if state.deny_list.is_denied(identity.as_str()).await {
        return Err(ApiError::unauthorized(
            "Access denied, wallet is deny-listed.",
        ));
    }
    let secret = auth.secret();
    let world = WorldName::parse(world_name).map_err(|e| ApiError::bad_request(e.to_string()))?;
    if world.display() != world_name {
        return Err(ApiError::bad_request("Invalid world name"));
    }
    let now = chrono::Utc::now().naive_utc();

    let access_type = state
        .policy
        .access_type(&world)
        .await
        .map_err(undetermined)?;
    let is_shared_secret = access_type == Some(AccessType::SharedSecret);
    let subject = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| identity.as_str().to_string());
    let rate_limit_key = format!("{world_name}:{subject}");
    if is_shared_secret && state.world_mint.limiter.is_rate_limited(&rate_limit_key) {
        return Err(ApiError::too_many(
            "Too many shared-secret attempts. Try again later.",
            RATE_LIMIT_WINDOW_SECONDS,
        ));
    }

    match crate::gatekeeper::moderation::admission_ban(
        state.policy.as_ref(),
        state.bans.as_ref(),
        &identity,
        None,
        None,
        now,
    )
    .await
    .map_err(undetermined)?
    {
        Some(Ban::Blocked | Ban::Platform) => {
            return Err(ApiError::unauthorized(
                "Access denied, you are banned from the platform.",
            ));
        }
        Some(Ban::Scene) | None => {}
    }

    let scene = match scene_id {
        Some(id) => Some(
            SceneId::parse(id)
                .ok()
                .filter(|scene| scene.as_str() == id)
                .ok_or_else(|| {
                    ApiError::not_found(format!(
                        "Scene \"{id}\" not found in world \"{world_name}\"."
                    ))
                })?,
        ),
        None => None,
    };
    let request = match &scene {
        Some(scene) => RoomRequest::WorldScene {
            world: &world,
            scene,
        },
        None => RoomRequest::WorldLobby { world: &world },
    };
    let resolved = match state.scenes.resolve(request).await {
        Ok(r) => r,
        Err(ResolveError::SceneNotInWorld { .. }) => {
            return Err(ApiError::not_found(format!(
                "Scene \"{}\" not found in world \"{world_name}\".",
                scene_id.unwrap_or_default()
            )));
        }
        Err(e) => return Err(undetermined(e)),
    };

    match state
        .policy
        .can_access(
            &world,
            &identity,
            secret.as_deref(),
            resolved.scene_id.as_ref(),
            None,
            now,
        )
        .await
        .map_err(undetermined)?
    {
        Access::Allowed(_) => {}
        Access::Banned(Ban::Scene) => {
            return Err(ApiError::unauthorized(format!(
                "You are banned from world \"{world_name}\"."
            )));
        }
        Access::Banned(_) => {
            return Err(ApiError::unauthorized(
                "Access denied, you are banned from the platform.",
            ));
        }
        Access::InvalidSecret => {
            state
                .world_mint
                .limiter
                .record_failed_attempt(&rate_limit_key);
            if state.world_mint.limiter.is_rate_limited(&rate_limit_key) {
                return Err(ApiError::too_many(
                    "Too many shared-secret attempts. Try again later.",
                    RATE_LIMIT_WINDOW_SECONDS,
                ));
            }
            return Err(ApiError::forbidden(format!(
                "Access denied, invalid secret for world \"{world_name}\"."
            )));
        }
        Access::NotAllowed => {
            return Err(ApiError::unauthorized(format!(
                "You are not allowed to access world \"{world_name}\"."
            )));
        }
    }

    let participants = state
        .presence
        .admission_occupancy(state, world_name, scene_id.is_some())
        .await
        .map_err(undetermined)?;
    if participants >= state.world_mint.max_users_per_world {
        return Err(ApiError::service_unavailable(format!(
            "World \"{world_name}\" has reached its maximum capacity."
        )));
    }
    if is_shared_secret {
        state.world_mint.limiter.clear(&rate_limit_key);
    }
    let token = AccessToken::new(
        &state.livekit.api_key,
        &state.livekit.api_secret,
        identity.as_str(),
        join_grants(resolved.room),
    )
    .to_jwt()
    .map_err(|e| ApiError::internal(format!("token mint failed: {e}")))?;
    Ok(Json(CommsAdapterResponse {
        fixed_adapter: build_adapter_url(&state.livekit.client_url, &token),
    }))
}

#[cfg(test)]
mod policy_boundary_tests {
    use super::*;
    use crate::interfaces::{AccessPolicy, test_support::StubPolicy};
    use crate::shared::identity::AuthorityNotEstablished;

    #[tokio::test]
    async fn access_setting_distinguishes_unknown_and_shared_secret_worlds() {
        let world = WorldName::parse("example.dcl.eth").unwrap();
        assert_eq!(
            StubPolicy::default().access_type(&world).await.unwrap(),
            None
        );
        let policy = StubPolicy {
            access_type: Ok(Some(AccessType::SharedSecret)),
            ..Default::default()
        };
        assert_eq!(
            policy.access_type(&world).await.unwrap(),
            Some(AccessType::SharedSecret)
        );
    }

    #[tokio::test]
    async fn access_authority_fault_keeps_retryable_503() {
        let policy = StubPolicy {
            access_type: Err(AuthorityNotEstablished::UndeterminedStoreUnavailable {
                store: "worlds.worlds",
                reason_for_operators: "unavailable".into(),
            }),
            ..Default::default()
        };
        let world = WorldName::parse("example.dcl.eth").unwrap();
        let error = policy
            .access_type(&world)
            .await
            .map_err(undetermined)
            .unwrap_err();
        assert_eq!(error.status(), 503);
        assert!(!error.public_message().contains("unavailable"));
    }
}
