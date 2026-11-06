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

/// One shared caller budget across world mint, adapter and scene-host entry points.
pub struct SecretThrottle<'a> {
    pub pool: &'a PgPool,
    pub world: &'a str,
    pub subject: &'a str,
    pub max_attempts: u32,
}

/// The exact database window charged by a reservation. Consumed by settlement.
pub struct SecretReservation {
    window: NaiveDateTime,
}

impl SecretThrottle<'_> {
    fn subject_key(&self) -> String {
        crate::shared::client_ip::source_key(self.subject).to_lowercase()
    }

    pub async fn reserve(
        &self,
        now: NaiveDateTime,
    ) -> Result<Option<SecretReservation>, sqlx::Error> {
        let window: Option<NaiveDateTime> = sqlx::query_scalar(
            "INSERT INTO social.world_secret_attempts AS a (world_name, subject, attempts, window_start) \
             VALUES ($1, $2, 1, $3) ON CONFLICT (world_name, subject) DO UPDATE SET \
             attempts = CASE WHEN a.window_start <= $4 THEN 1 ELSE a.attempts + 1 END, \
             window_start = CASE WHEN a.window_start <= $4 THEN $3 ELSE a.window_start END \
             WHERE a.window_start <= $4 OR a.attempts < $5 RETURNING window_start")
            .bind(self.world).bind(self.subject_key()).bind(now).bind(window_start_cutoff(now))
            .bind(self.max_attempts.clamp(1, i32::MAX as u32) as i32).fetch_optional(self.pool).await?;
        Ok(window.map(|window| SecretReservation { window }))
    }

    pub async fn settle(&self, invalid_secret: bool, reservation: SecretReservation) {
        if invalid_secret {
            return;
        }
        let result = async {
            let functions = super::functions::installed(self.pool).await?;
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "SELECT social.{}($1,$2,$3)",
                functions.refund
            )))
            .bind(self.world)
            .bind(self.subject_key())
            .bind(reservation.window)
            .execute(self.pool)
            .await
        }
        .await;
        if let Err(error) = result {
            tracing::warn!(%error, world=self.world, "world secret reservation refund failed; charge expires with its window");
        }
    }
}

/// Drops expired throttle windows (the `comms-secret-attempts-sweep` job).
pub async fn sweep_secret_attempts(pool: &PgPool, now: NaiveDateTime) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM social.world_secret_attempts WHERE (world_name,subject) IN (SELECT world_name,subject FROM social.world_secret_attempts WHERE window_start <= $1 ORDER BY window_start LIMIT 1000 FOR UPDATE SKIP LOCKED)")
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

/// Stamp authorization after any already-started access edit commits. A short
/// shared row lock orders this clock read against the edit's exclusive row lock;
/// no network or policy calls run while the lock is held.
pub(crate) async fn authorization_started(
    pool: &PgPool,
    world: &WorldName,
) -> Result<NaiveDateTime, sqlx::Error> {
    let functions = super::functions::installed(pool).await?;
    let started: Option<NaiveDateTime> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT social.{}($1)",
        functions.stamp
    )))
    .bind(world.key())
    .fetch_one(pool)
    .await?;
    started.ok_or(sqlx::Error::RowNotFound)
}

pub(crate) const AUTHORIZATION_FIELD: &str = "worldAuthorization";

fn authorization_mac(
    secret: &str,
    world: &WorldName,
    identity: &str,
    room: &str,
    at: i64,
) -> hmac::Hmac<sha2::Sha256> {
    use hmac::Mac;
    let mut mac = <hmac::Hmac<sha2::Sha256> as hmac::KeyInit>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(b"dcl-world-authorization-v1\0");
    mac.update(
        serde_json::json!([world.key(), identity, room, at])
            .to_string()
            .as_bytes(),
    );
    mac
}

/// An independent signature is necessary: LiveKit participant metadata can be
/// changed by clients and is also copied into refreshed connection tokens.
pub(crate) fn authorization_proof(
    secret: &str,
    world: &WorldName,
    identity: &str,
    room: &str,
    started: NaiveDateTime,
) -> serde_json::Value {
    use hmac::Mac;
    let at = started.and_utc().timestamp_micros();
    let signature = hex::encode(
        authorization_mac(secret, world, identity, room, at)
            .finalize()
            .into_bytes(),
    );
    serde_json::json!({"at": at, "signature": signature})
}

pub(crate) fn authorized_after(
    secret: &str,
    world: &WorldName,
    identity: &str,
    room: &str,
    metadata: Option<&str>,
    cutoff: NaiveDateTime,
) -> bool {
    use hmac::Mac;
    let Some(proof) = metadata
        .filter(|raw| raw.len() <= 16_384)
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|value| value.get(AUTHORIZATION_FIELD).cloned())
    else {
        return false;
    };
    let Some(at) = proof.get("at").and_then(|value| value.as_i64()) else {
        return false;
    };
    let Some(signature) = proof
        .get("signature")
        .and_then(|value| value.as_str())
        .and_then(|value| hex::decode(value).ok())
    else {
        return false;
    };
    at > cutoff.and_utc().timestamp_micros()
        && authorization_mac(secret, world, identity, room, at)
            .verify_slice(&signature)
            .is_ok()
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
    let attempts = SecretThrottle {
        pool: &state.pool,
        world: world.key(),
        subject: &subject,
        max_attempts: state.world_mint.max_secret_attempts,
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

    let started = authorization_started(&state.pool, &world)
        .await
        .map_err(undetermined)?;

    // Reserved before the secret is checked, so concurrent wrong secrets cannot all reach bcrypt.
    let reservation = if is_shared_secret {
        Some(
            attempts
                .reserve(now)
                .await
                .map_err(undetermined)?
                .ok_or_else(too_many_attempts)?,
        )
    } else {
        None
    };
    let access = state
        .policy
        .can_access(
            &world,
            &identity,
            secret.as_deref(),
            resolved.scene_id.as_ref(),
            None,
            now,
        )
        .await;
    if let Some(reservation) = reservation {
        attempts
            .settle(matches!(access, Ok(Access::InvalidSecret)), reservation)
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
    let proof = authorization_proof(
        &state.livekit.api_secret,
        &world,
        identity.as_str(),
        &resolved.room,
        started,
    );
    let token = AccessToken::new(
        &state.livekit.api_key,
        &state.livekit.api_secret,
        identity.as_str(),
        join_grants(resolved.room),
    )
    .with_metadata(json!({AUTHORIZATION_FIELD: proof}).to_string())
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

    #[test]
    fn authorization_proof_is_bound_to_world_wallet_room_and_cutoff() {
        let world = WorldName::parse("proof.dcl.eth").unwrap();
        let other = WorldName::parse("other.dcl.eth").unwrap();
        let cutoff = chrono::Utc::now().naive_utc();
        let proof = authorization_proof(
            "key",
            &world,
            "wallet",
            "room",
            cutoff + chrono::Duration::microseconds(1),
        );
        let metadata = serde_json::json!({AUTHORIZATION_FIELD: proof}).to_string();
        assert!(
            serde_json::from_str::<serde_json::Value>(&metadata)
                .unwrap()
                .get("worldAuthorization")
                .is_some()
        );
        assert!(authorized_after(
            "key",
            &world,
            "wallet",
            "room",
            Some(&metadata),
            cutoff
        ));
        assert!(!authorized_after(
            "key",
            &world,
            "wallet",
            "room",
            Some(&metadata),
            cutoff + chrono::Duration::microseconds(1)
        ));
        for (secret, world, identity, room) in [
            ("wrong", &world, "wallet", "room"),
            ("key", &other, "wallet", "room"),
            ("key", &world, "other", "room"),
            ("key", &world, "wallet", "other"),
        ] {
            assert!(!authorized_after(
                secret,
                world,
                identity,
                room,
                Some(&metadata),
                cutoff
            ));
        }
        let mut tampered = serde_json::from_str::<serde_json::Value>(&metadata).unwrap();
        tampered[AUTHORIZATION_FIELD]["at"] =
            serde_json::json!(cutoff.and_utc().timestamp_micros() + 2);
        for metadata in [
            None,
            Some("{}"),
            Some("not-json"),
            Some(tampered.to_string().as_str()),
        ] {
            assert!(!authorized_after(
                "key", &world, "wallet", "room", metadata, cutoff
            ));
        }
    }

    #[tokio::test]
    async fn authorization_stamp_waits_for_an_inflight_world_edit() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = WorldName::parse(&format!("stamp-{}.dcl.eth", uuid::Uuid::new_v4())).unwrap();
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES($1)")
            .bind(world.key())
            .execute(&pool)
            .await
            .unwrap();
        let mut edit = pool.begin().await.unwrap();
        sqlx::query("SELECT name FROM worlds.worlds WHERE name=$1 FOR UPDATE")
            .bind(world.key())
            .fetch_one(&mut *edit)
            .await
            .unwrap();
        let cutoff: NaiveDateTime = sqlx::query_scalar("SELECT timezone('UTC',clock_timestamp())")
            .fetch_one(&mut *edit)
            .await
            .unwrap();
        let (cloned_pool, cloned_world) = (pool.clone(), world.clone());
        let stamp =
            tokio::spawn(async move { authorization_started(&cloned_pool, &cloned_world).await });
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        assert!(
            !stamp.is_finished(),
            "stamp must wait for the accepted edit"
        );
        edit.commit().await.unwrap();
        assert!(stamp.await.unwrap().unwrap() >= cutoff);
    }

    /// Concurrent attempts each reserve atomically: only `max` get past the throttle.
    #[tokio::test]
    async fn concurrent_attempts_reserve_one_count_each() {
        let Some(pool) = crate::db::scratch::pool(12).await else {
            return;
        };
        let world = format!("throttle-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        let now = chrono::Utc::now().naive_utc();
        let attempts = SecretThrottle {
            pool: &pool,
            world: &world,
            subject: "203.0.113.7",
            max_attempts: 3,
        };
        let results = futures::future::join_all((0..10).map(|_| attempts.reserve(now))).await;
        assert_eq!(
            results
                .iter()
                .filter(|r| r.as_ref().unwrap().is_none())
                .count(),
            7
        );
        for reservation in results.into_iter().filter_map(Result::unwrap) {
            attempts.settle(false, reservation).await;
        }
        assert!(attempts.reserve(now).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn refunds_preserve_other_failures_and_replacement_windows() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("refund-{}.dcl.eth", uuid::Uuid::new_v4());
        let now = chrono::Utc::now().naive_utc();
        let throttle = SecretThrottle {
            pool: &pool,
            world: &world,
            subject: "203.0.113.9",
            max_attempts: 3,
        };
        let failed = throttle.reserve(now).await.unwrap().unwrap();
        let success = throttle
            .reserve(now - chrono::Duration::seconds(1))
            .await
            .unwrap()
            .unwrap();
        throttle.settle(true, failed).await;
        throttle.settle(false, success).await;
        let count: i32 = sqlx::query_scalar(
            "SELECT attempts FROM social.world_secret_attempts WHERE world_name=$1",
        )
        .bind(&world)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        let old = throttle.reserve(now).await.unwrap().unwrap();
        let replacement = throttle
            .reserve(now + chrono::Duration::seconds(61))
            .await
            .unwrap()
            .unwrap();
        throttle.settle(false, old).await;
        let count: i32 = sqlx::query_scalar(
            "SELECT attempts FROM social.world_secret_attempts WHERE world_name=$1",
        )
        .bind(&world)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        throttle.settle(false, replacement).await;
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.world_secret_attempts WHERE world_name=$1",
        )
        .bind(&world)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn stable_ban_upgrade_consolidates_active_retired_and_existing_keys() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("migration-{}.dcl.eth", uuid::Uuid::new_v4());
        let key = format!("{world}:1,2");
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES($1)")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO worlds.world_scenes(world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size) VALUES($1,'active','[]','{}','owner',ARRAY['01, 002'],1)").bind(&world).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO worlds.retired_world_scenes(world_name,entity_id,entity,parcels) VALUES($1,'retired','{}',ARRAY['1,2'])").bind(&world).execute(&pool).await.unwrap();
        for (id, age) in [("active", 1), ("retired", 2), (key.as_str(), 3)] {
            sqlx::query("INSERT INTO social.scene_bans(place_id,banned_address,banned_by,banned_at) VALUES($1,'wallet',$1,timezone('UTC',now())-$2*interval '1 second')").bind(id).bind(age).execute(&pool).await.unwrap();
        }
        let migration = include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/schemas/migrations/0029_world_scene_ban_keys.sql"
        ));
        for _ in 0..2 {
            sqlx::raw_sql(migration).execute(&pool).await.unwrap();
        }
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT place_id,banned_by FROM social.scene_bans WHERE banned_address='wallet'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows, vec![(key.clone(), "active".into())]);
        sqlx::query("DELETE FROM social.scene_bans WHERE place_id=$1")
            .bind(key)
            .execute(&pool)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.scene_bans WHERE banned_address='wallet'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "unban must not leave an effective legacy alias");
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
