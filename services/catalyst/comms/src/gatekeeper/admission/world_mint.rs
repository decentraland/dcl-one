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
//! shared-secret world's secret attempts are throttled per world and caller
//! (`SHARED_SECRET_MAX_ATTEMPTS_PER_MINUTE`, 10) in Postgres, so every
//! replica shares one budget. An unknown, operator-disabled or deny-listed
//! world answers 404. The old explorer-api shim proxied the two `/worlds/...` paths
//! here; they are these routes now. The old service's external wallet
//! denylist is checked through the shared DENY_LIST_URL reader, alongside
//! the operator's block list, platform and scene bans.

use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Uri};
use axum::routing::post;
use axum::{Json, Router};
use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::PgPool;

use crate::access::{Access, AccessType, Ban, WorldStanding};
use crate::gatekeeper::SharedState;
use crate::gatekeeper::livekit::{AccessToken, build_adapter_url, join_grants};
use crate::limits::RateLimiter;
use crate::rooms::{ResolveError, RoomRequest, WORLD_ROOM_PREFIX, worlds_service_scene_room};
use crate::shared::signed_fetch::SignedPath;
use crate::signed_fetch::{EXPLORER_METADATA_KEYS, map_auth_error_api, require_verified};
use crate::world_identity::{SceneId, WorldName};
use crate::worlds_envelope::ApiError;

pub const DEFAULT_SHARED_SECRET_MAX_ATTEMPTS: u32 = 10;
pub const RATE_LIMIT_WINDOW_SECONDS: u64 = 60;
pub const DEFAULT_MAX_USERS_PER_WORLD: i64 = 100;

/// The mints' settings: the shared-secret budget and the capacity.
pub struct MintState {
    pub(crate) secret_max_attempts: u32,
    pub(crate) max_users_per_world: i64,
}

impl Default for MintState {
    fn default() -> Self {
        Self {
            secret_max_attempts: DEFAULT_SHARED_SECRET_MAX_ATTEMPTS,
            max_users_per_world: DEFAULT_MAX_USERS_PER_WORLD,
        }
    }
}

/// Shared-secret attempts per (world, caller) in one fixed window, shared by
/// every replica through `social.world_secret_attempts`: an attempt is
/// reserved (`in_flight`) before the secret is checked, so parallel guesses
/// spend the same budget; a wrong secret turns its reservation into a
/// failure and a correct one only returns it. An attempt that finds part of
/// the budget held by reservations still in flight waits for them to settle. A
/// store fault admits the attempt (the old Redis limiter failed open too).
pub(crate) struct SecretThrottle<'a> {
    pub pool: &'a PgPool,
    pub max_attempts: u32,
    pub world: &'a str,
    pub subject: &'a str,
}

/// Expired windows each reservation purges at most, skipping locked rows.
const PURGE_BATCH: i64 = 32;
/// How long an attempt waits for in-flight reservations before a 429.
const SETTLE_WAIT: std::time::Duration = std::time::Duration::from_secs(2);
const SETTLE_POLL: std::time::Duration = std::time::Duration::from_millis(20);

impl SecretThrottle<'_> {
    fn window_start(now: NaiveDateTime) -> NaiveDateTime {
        now - chrono::Duration::seconds(RATE_LIMIT_WINDOW_SECONDS as i64)
    }

    /// Reserves one attempt; `true` when the window's failures hold the
    /// budget, or its reservations still do once `SETTLE_WAIT` passed.
    /// Settle every admitted attempt.
    pub async fn reserve(&self, now: NaiveDateTime) -> bool {
        let deadline = tokio::time::Instant::now() + SETTLE_WAIT;
        loop {
            match self.try_reserve(now).await {
                Ok(None) => return false,
                Ok(Some(failures)) if failures >= self.max_attempts as i32 => return true,
                // Reservations in flight hold the rest: wait for them to settle.
                Ok(Some(_)) if tokio::time::Instant::now() < deadline => {
                    tokio::time::sleep(SETTLE_POLL).await;
                }
                Ok(Some(_)) => return true,
                Err(error) => {
                    tracing::warn!(%error, world = self.world, "shared-secret attempt not recorded; admitting");
                    return false;
                }
            }
        }
    }

    /// `None` when reserved, else the window's settled failures.
    async fn try_reserve(&self, now: NaiveDateTime) -> Result<Option<i32>, sqlx::Error> {
        let reserved = sqlx::query_scalar::<_, i32>(
            "WITH purge AS ( \
               DELETE FROM social.world_secret_attempts \
                WHERE (world_name, subject) IN ( \
                  SELECT world_name, subject FROM social.world_secret_attempts \
                   WHERE window_started_at <= $4 AND (world_name, subject) <> ($1, $2) \
                   LIMIT $5 FOR UPDATE SKIP LOCKED)) \
             INSERT INTO social.world_secret_attempts AS a (world_name, subject, window_started_at, failures, in_flight) \
             VALUES ($1, $2, $3, 0, 1) \
             ON CONFLICT (world_name, subject) DO UPDATE SET \
               window_started_at = CASE WHEN a.window_started_at <= $4 THEN $3 ELSE a.window_started_at END, \
               failures = CASE WHEN a.window_started_at <= $4 THEN 0 ELSE a.failures END, \
               in_flight = CASE WHEN a.window_started_at <= $4 THEN 1 ELSE a.in_flight + 1 END \
             WHERE a.window_started_at <= $4 OR a.failures + a.in_flight < $6 \
             RETURNING in_flight",
        )
        .bind(self.world)
        .bind(self.subject.to_lowercase())
        .bind(now)
        .bind(Self::window_start(now))
        .bind(PURGE_BATCH)
        .bind(self.max_attempts as i32)
        .fetch_optional(self.pool)
        .await?;
        if reserved.is_some() {
            return Ok(None);
        }
        let failures = sqlx::query_scalar::<_, i32>(
            "SELECT failures FROM social.world_secret_attempts WHERE world_name = $1 AND subject = $2",
        )
        .bind(self.world)
        .bind(self.subject.to_lowercase())
        .fetch_optional(self.pool)
        .await?;
        Ok(Some(failures.unwrap_or(0)))
    }

    /// Settles a reservation: a correct secret refunds it, any other outcome
    /// keeps it as a failure. Never resets the window for others.
    pub async fn settle(&self, correct: bool) {
        if let Err(error) = sqlx::query(
            "UPDATE social.world_secret_attempts \
                SET in_flight = GREATEST(in_flight - 1, 0), \
                    failures = failures + CASE WHEN $3 THEN 0 ELSE 1 END \
              WHERE world_name = $1 AND subject = $2",
        )
        .bind(self.world)
        .bind(self.subject.to_lowercase())
        .bind(correct)
        .execute(self.pool)
        .await
        {
            tracing::warn!(%error, world = self.world, "shared-secret attempt not settled");
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

    let access_type = match state
        .policy
        .world_standing(&world)
        .await
        .map_err(undetermined)?
    {
        WorldStanding::Open(access_type) => access_type,
        WorldStanding::Unknown | WorldStanding::Disabled | WorldStanding::DenyListed => {
            return Err(ApiError::not_found(if explorer_handshake {
                format!("World \"{world_name}\" was not found.")
            } else {
                format!("World \"{world_name}\" is invalid or blocked.")
            }));
        }
    };
    let is_shared_secret = access_type == AccessType::SharedSecret;
    let subject = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(identity.as_str());
    let throttle = SecretThrottle {
        pool: &state.pool,
        max_attempts: state.world_mint.secret_max_attempts,
        world: world.key(),
        subject,
    };

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
    let not_found = || {
        ApiError::not_found(format!(
            "Scene \"{}\" not found in world \"{world_name}\".",
            scene_id.unwrap_or_default()
        ))
    };
    let (room, ban_key, successor_keys) = match state.scenes.resolve(request).await {
        Ok(r) => {
            let ban_key = r.ban_key();
            (r.room, ban_key, Vec::new())
        }
        // A scene a redeploy just replaced keeps its room a bounded window.
        Err(ResolveError::SceneNotInWorld { .. }) => {
            let scene = scene.as_ref().ok_or_else(not_found)?;
            let replaced = state
                .scenes
                .replaced_world_scene(&world, scene)
                .await
                .map_err(undetermined)?
                .ok_or_else(not_found)?;
            let mut keys = replaced.ban_keys(&world).into_iter();
            let own = keys.next();
            (
                worlds_service_scene_room(&world, scene),
                own,
                keys.collect(),
            )
        }
        Err(e) => return Err(undetermined(e)),
    };
    // A ban on the scene that replaced this one reaches its room too.
    for key in successor_keys
        .iter()
        .filter(|key| Some(*key) != ban_key.as_ref())
    {
        if crate::gatekeeper::moderation::admission_ban(
            state.policy.as_ref(),
            state.bans.as_ref(),
            &identity,
            Some(key),
            None,
            now,
        )
        .await
        .map_err(undetermined)?
        .is_some()
        {
            return Err(ApiError::unauthorized(format!(
                "You are banned from world \"{world_name}\"."
            )));
        }
    }
    if is_shared_secret && throttle.reserve(now).await {
        return Err(ApiError::too_many(
            "Too many shared-secret attempts. Try again later.",
            RATE_LIMIT_WINDOW_SECONDS,
        ));
    }

    let access = state
        .policy
        .can_access(
            &world,
            &identity,
            secret.as_deref(),
            ban_key.as_ref(),
            None,
            now,
        )
        .await;
    if is_shared_secret {
        throttle
            .settle(matches!(access, Ok(Access::Allowed(_))))
            .await;
    }
    match access.map_err(undetermined)? {
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
    let token = AccessToken::new(
        &state.livekit.api_key,
        &state.livekit.api_secret,
        identity.as_str(),
        join_grants(room),
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
    async fn access_authority_fault_keeps_retryable_503() {
        let policy = StubPolicy {
            standing: Err(AuthorityNotEstablished::UndeterminedStoreUnavailable {
                store: "worlds.worlds",
                reason_for_operators: "unavailable".into(),
            }),
            ..Default::default()
        };
        let world = WorldName::parse("example.dcl.eth").unwrap();
        let error = policy
            .world_standing(&world)
            .await
            .map_err(undetermined)
            .unwrap_err();
        assert_eq!(error.status(), 503);
        assert!(!error.public_message().contains("unavailable"));
    }
}
