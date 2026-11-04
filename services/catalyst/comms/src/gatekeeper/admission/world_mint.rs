//! The worlds service's comms mints: `POST /worlds/{name}/comms` (the world
//! lobby), `POST /worlds/{name}/scenes/{id}/comms` (a scene deployed in the
//! world) and `POST /get-comms-adapter/world-{name}` (the lobby by room id),
//! at the root and under `/worlds-content-server`, answering in the worlds
//! envelope (`crate::worlds_envelope`). `worlds::core` assigns them to
//! comms, which owns the LiveKit transport: the room is the worlds service's
//! contract (`rooms`: the lobby `world-<key>`, a scene
//! `w1:<base36 key length>:<key><scene id>`, resolved by `SceneIdentity`),
//! the decision the ONE permission core's (`AccessPolicy::world_admission`:
//! an unknown world or one blocked past its grace is 404;
//! `AccessPolicy::is_banned`, `AccessPolicy::can_access`), the capacity the
//! live presence's (`crate::presence`, `MAX_USERS_PER_WORLD`, 100), and a
//! shared-secret world's wrong secrets are throttled per world and caller
//! (`SHARED_SECRET_MAX_ATTEMPTS_PER_MINUTE`, 3) in Postgres, so every replica
//! shares one count. The old explorer-api shim proxied the two `/worlds/...`
//! paths here; they are these routes now. The old service's external wallet
//! denylist is checked through the shared DENY_LIST_URL reader, alongside
//! the operator's block list, platform and scene bans.

use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Uri};
use axum::routing::post;
use axum::{Json, Router};
use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::PgPool;

use crate::access::{Access, AccessType, Ban, WorldAdmission};
use crate::gatekeeper::SharedState;
use crate::gatekeeper::livekit::{AccessToken, build_adapter_url, join_grants};
use crate::rooms::{ResolveError, RoomRequest, WORLD_ROOM_PREFIX};
use crate::shared::signed_fetch::SignedPath;
use crate::signed_fetch::{EXPLORER_METADATA_KEYS, map_auth_error_api, require_verified};
use crate::world_identity::{SceneId, WorldName};
use crate::worlds_envelope::ApiError;

pub const DEFAULT_SHARED_SECRET_MAX_ATTEMPTS: u32 = 3;
pub const RATE_LIMIT_WINDOW_SECONDS: u64 = 60;
pub const DEFAULT_MAX_USERS_PER_WORLD: i64 = 100;

/// The mints' settings: the shared-secret throttle and the capacity.
pub struct MintState {
    pub(crate) max_secret_attempts: u32,
    pub(crate) max_users_per_world: i64,
}

impl Default for MintState {
    fn default() -> Self {
        Self {
            max_secret_attempts: DEFAULT_SHARED_SECRET_MAX_ATTEMPTS,
            max_users_per_world: DEFAULT_MAX_USERS_PER_WORLD,
        }
    }
}

fn window_start_cutoff(now: NaiveDateTime) -> NaiveDateTime {
    now - chrono::TimeDelta::seconds(RATE_LIMIT_WINDOW_SECONDS as i64)
}

/// Wrong shared secrets of one caller for one world, in
/// `social.world_secret_attempts` (a fixed window from the first failure).
struct SecretAttempts<'a> {
    pool: &'a PgPool,
    world: &'a str,
    subject: String,
    max: u32,
}

impl SecretAttempts<'_> {
    async fn is_limited(&self, now: NaiveDateTime) -> Result<bool, sqlx::Error> {
        let attempts: Option<i32> = sqlx::query_scalar(
            "SELECT attempts FROM social.world_secret_attempts \
             WHERE world_name = $1 AND subject = $2 AND window_start > $3",
        )
        .bind(self.world)
        .bind(&self.subject)
        .bind(window_start_cutoff(now))
        .fetch_optional(self.pool)
        .await?;
        Ok(attempts.is_some_and(|n| n.max(0) as u32 >= self.max))
    }

    /// Counts a failure; `true` when it went past the limit (WCS: the
    /// limit-th failure still answers 403, the next request 429).
    async fn record_failure(&self, now: NaiveDateTime) -> Result<bool, sqlx::Error> {
        let attempts: i32 = sqlx::query_scalar(
            "INSERT INTO social.world_secret_attempts AS a (world_name, subject, attempts, window_start) \
             VALUES ($1, $2, 1, $3) \
             ON CONFLICT (world_name, subject) DO UPDATE SET \
               attempts = CASE WHEN a.window_start <= $4 THEN 1 ELSE a.attempts + 1 END, \
               window_start = CASE WHEN a.window_start <= $4 THEN $3 ELSE a.window_start END \
             RETURNING attempts",
        )
        .bind(self.world)
        .bind(&self.subject)
        .bind(now)
        .bind(window_start_cutoff(now))
        .fetch_one(self.pool)
        .await?;
        Ok(attempts.max(0) as u32 > self.max)
    }

    async fn clear(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM social.world_secret_attempts WHERE world_name = $1 AND subject = $2",
        )
        .bind(self.world)
        .bind(&self.subject)
        .execute(self.pool)
        .await
        .map(|_| ())
    }
}

/// Drops expired throttle windows (the `comms-secret-attempts-sweep` job).
pub async fn sweep_secret_attempts(pool: &PgPool, now: NaiveDateTime) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM social.world_secret_attempts WHERE window_start <= $1")
        .bind(window_start_cutoff(now))
        .execute(pool)
        .await?;
    Ok(())
}

fn too_many_attempts() -> ApiError {
    ApiError::too_many(
        "Too many shared-secret attempts. Try again later.",
        RATE_LIMIT_WINDOW_SECONDS,
    )
}

fn invalid_world(world_name: &str) -> ApiError {
    ApiError::not_found(format!("World \"{world_name}\" is invalid or blocked."))
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

    let admission = state
        .policy
        .world_admission(&world, now)
        .await
        .map_err(undetermined)?;
    let is_shared_secret = admission == WorldAdmission::Open(AccessType::SharedSecret);
    let subject = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| identity.as_str().to_string());
    let attempts = SecretAttempts {
        pool: &state.pool,
        world: world.key(),
        subject: subject.to_lowercase(),
        max: state.world_mint.max_secret_attempts,
    };
    if is_shared_secret && attempts.is_limited(now).await.map_err(undetermined)? {
        return Err(too_many_attempts());
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
    if matches!(admission, WorldAdmission::Unknown | WorldAdmission::Blocked) {
        return Err(invalid_world(world_name));
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
        // The row vanished since `world_admission`: still an unknown world.
        Err(ResolveError::UnresolvedRoom(_)) => return Err(invalid_world(world_name)),
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
            if attempts.record_failure(now).await.map_err(undetermined)? {
                return Err(too_many_attempts());
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
        attempts.clear().await.map_err(undetermined)?;
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
