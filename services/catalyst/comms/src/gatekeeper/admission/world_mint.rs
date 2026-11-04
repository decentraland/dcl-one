//! The worlds service's comms mints: `POST /worlds/{name}/comms` (the world
//! lobby), `POST /worlds/{name}/scenes/{id}/comms` (a scene deployed in the
//! world) and `POST /get-comms-adapter/world-{name}` (the lobby by room id),
//! at the root and under `/worlds-content-server`, answering in the worlds
//! envelope (`crate::worlds_envelope`) with the reason in `error` where WCS's
//! comms handlers put it there. `worlds::core` assigns them to
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
use axum::http::{HeaderMap, Uri, header::CONTENT_LENGTH};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use chrono::NaiveDateTime;
use serde::Serialize;
use serde_json::json;
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
    /// Reserves one attempt before the secret is checked; `true` when it went past the limit
    /// (WCS: the limit-th wrong secret still answers 403, the next request 429).
    async fn reserve(&self, now: NaiveDateTime) -> Result<bool, sqlx::Error> {
        let attempts: i32 = sqlx::query_scalar(
            "INSERT INTO social.world_secret_attempts AS a (world_name, subject, attempts, window_start) \
             VALUES ($1, $2, 1, $3) \
             ON CONFLICT (world_name, subject) DO UPDATE SET \
               attempts = CASE WHEN a.window_start <= $4 THEN 1 ELSE LEAST(a.attempts, 1000000) + 1 END, \
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

    /// Best effort: a failure only leaves the caller's count to expire.
    async fn clear(&self) {
        if let Err(error) = sqlx::query(
            "DELETE FROM social.world_secret_attempts WHERE world_name = $1 AND subject = $2",
        )
        .bind(self.world)
        .bind(&self.subject)
        .execute(self.pool)
        .await
        {
            tracing::warn!(%error, world = self.world, "world comms mint: could not clear the secret attempts");
        }
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
) -> Response {
    let path = SignedPath::from_uris(&original, &uri);
    answer(
        mint(&state, &world_name, None, &path, &headers, false).await,
        WORLD_ROUTE_REASONS,
    )
}

async fn get_comms_adapter(
    State(state): State<SharedState>,
    Path(room_id): Path<String>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let Some(world_name) = room_id.strip_prefix(WORLD_ROOM_PREFIX) else {
        return ApiError::bad_request("Invalid room id requested.").into_response();
    };
    let path = SignedPath::from_uris(&original, &uri);
    answer(
        mint(&state, world_name, None, &path, &headers, true).await,
        ADAPTER_ROUTE_REASONS,
    )
}

async fn world_scene_comms(
    State(state): State<SharedState>,
    Path((world_name, scene_id)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let path = SignedPath::from_uris(&original, &uri);
    answer(
        mint(&state, &world_name, Some(&scene_id), &path, &headers, false).await,
        WORLD_ROUTE_REASONS,
    )
}

/// Statuses WCS's `world-comms-handler` answers as `{error: <reason>}`.
const WORLD_ROUTE_REASONS: &[u16] = &[401, 403, 404, 429, 503];
/// `comms-adapter-handler` throws http-commons errors except its 429.
const ADAPTER_ROUTE_REASONS: &[u16] = &[429];

/// The mint's answer: the listed statuses carry the reason in `error` (and in `message`).
fn answer(result: Result<Json<CommsAdapterResponse>, ApiError>, reasons: &[u16]) -> Response {
    let error = match result {
        Ok(json) => return json.into_response(),
        Err(error) => error,
    };
    let reason = (!matches!(error, ApiError::Internal(_)) && reasons.contains(&error.status()))
        .then(|| error.public_message());
    let mut response = error.into_response();
    if let Some(reason) = reason {
        response.headers_mut().remove(CONTENT_LENGTH);
        *response.body_mut() =
            axum::body::Body::from(json!({ "error": reason, "message": reason }).to_string());
    }
    response
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
    let ip = crate::shared::client_ip::client_ip(headers);
    let subject = if ip.is_empty() {
        identity.as_str().to_string()
    } else {
        crate::shared::client_ip::source_key(&ip)
    };
    let attempts = SecretAttempts {
        pool: &state.pool,
        world: world.key(),
        subject: subject.to_lowercase(),
        max: state.world_mint.max_secret_attempts,
    };

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
        // The lobby: the row vanished since `world_admission`, still an unknown world.
        Err(ResolveError::UnresolvedRoom(_)) if scene_id.is_none() => {
            return Err(invalid_world(world_name));
        }
        Err(ResolveError::UnresolvedRoom(_)) => {
            return Err(ApiError::not_found(format!(
                "The comms room of scene \"{}\" in world \"{world_name}\" could not be resolved.",
                scene_id.unwrap_or_default()
            )));
        }
        Err(e) => return Err(undetermined(e)),
    };

    // Reserved before the secret is checked, so concurrent wrong secrets cannot all reach bcrypt.
    if is_shared_secret && attempts.reserve(now).await.map_err(undetermined)? {
        return Err(too_many_attempts());
    }
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
        Access::Allowed(_) => {
            if is_shared_secret {
                attempts.clear().await;
            }
        }
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

#[cfg(test)]
mod throttle_tests {
    use super::*;

    /// Concurrent attempts each reserve atomically: only `max` get past the throttle.
    #[tokio::test]
    async fn concurrent_attempts_reserve_one_count_each() {
        let Some(pool) = crate::db::scratch::pool(12).await else {
            return;
        };
        let world = format!("throttle-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        let now = chrono::Utc::now().naive_utc();
        let attempts = SecretAttempts {
            pool: &pool,
            world: &world,
            subject: "203.0.113.7".into(),
            max: 3,
        };
        let limited = futures::future::join_all((0..10).map(|_| attempts.reserve(now))).await;
        let refused = limited.iter().filter(|r| *r.as_ref().unwrap()).count();
        assert_eq!(refused, 7);
        attempts.clear().await;
        assert!(
            !attempts.reserve(now).await.unwrap(),
            "a right secret resets the count"
        );
    }

    #[tokio::test]
    async fn mint_refusals_carry_the_reason_in_error() {
        let body = |response: Response| async move {
            let bytes = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()
        };
        let response = answer(Err(ApiError::unauthorized("nope")), WORLD_ROUTE_REASONS);
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        assert_eq!(
            body(response).await,
            json!({ "error": "nope", "message": "nope" })
        );
        let response = answer(Err(too_many_attempts()), ADAPTER_ROUTE_REASONS);
        assert_eq!(response.headers()["retry-after"], "60");
        assert_eq!(
            body(response).await["error"],
            "Too many shared-secret attempts. Try again later."
        );
        let response = answer(Err(ApiError::unauthorized("nope")), ADAPTER_ROUTE_REASONS);
        assert_eq!(body(response).await["error"], "Not Authorized");
    }
}
