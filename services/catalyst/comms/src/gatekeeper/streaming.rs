//! `/scene-stream-access` (GET/POST/PUT/DELETE, scene signed): the RTMP
//! ingress credential of a place's stream. The signer must own or
//! administer the place the signed metadata stands on. List, add and reset
//! require the signed scene to be the current deployment on its parcel and
//! authorize against that deployment: its footprint decides ownership and its
//! place key finds the explicit admins `/scene-admin` granted, while the
//! credential is stored for the place based on its base parcel. A world has
//! one credential for all its scenes, so there only authority over the whole
//! world counts: its owner, an admin of the world place, a deployment or
//! streaming grantee without a parcel restriction. Remove needs
//! only the place (a world by its realm, a Genesis place by its
//! parcel), so a key can still be revoked after a redeploy, by the admins of
//! the scene standing there now. List, add and reset are refused to
//! a platform-banned wallet BEFORE that check, because LiveKit honours the
//! key without re-checking the wallet, and remove stays ungated (upstream's
//! `*-scene-stream-access-handler.ts`). One active row per place in
//! `social.scene_stream_access`; the ingress itself is LiveKit's (twirp
//! `livekit.Ingress`), and the LiveKit webhook keeps the active row's
//! `streaming` flag. The Cast 2.0 routes (`/cast/*`, [`cast`]) mint and read
//! the same rows.

use crate::shared::now_utc;
use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::admission::scene::{self, PlaceLookup, meta_str, realm_name_from_metadata};
use super::auth::{SCENE_SIGNER, device_identifier, verify_signed_fetch};
use super::errors::{ApiError, forbidden, service_unavailable};
use super::livekit::IngressClient;
use super::moderation::context::Context;
use super::moderation::store::PLATFORM_BANNED_MSG;
use super::{CommsState, SharedState};
use crate::rooms::{ResolvedRoom, RoomRequest};
use crate::shared::signed_fetch::SignedPath;
use crate::world_identity::{Parcel, RealmId, WorldName};
use crate::{Address, AuthorityNotEstablished, PermissionDecision};

#[path = "streaming_notifications.rs"]
pub(super) mod notifications;

#[path = "streaming_jobs.rs"]
mod jobs;

#[path = "streaming_cast.rs"]
pub(super) mod cast;
pub(super) use jobs::spawn_jobs;

/// The durable cleanup of one intent, for tests that retry it after a failure.
/// The job's drain would also take the intents of other tests, which share
/// the database. As in the drain, a failed delivery stays for a retry.
#[cfg(test)]
pub(crate) async fn drain_cleanup_for_test(state: &CommsState, id: Uuid) {
    let _ = jobs::cleanup_access(state, id).await;
}

/// The streaming job's pass, for tests with a database of their own: the
/// reconciliation, then the four-hour cap of `id`. Returns whether the cap cut it.
#[cfg(test)]
pub(crate) async fn ttl_pass_for_test(state: &CommsState, id: Uuid) -> bool {
    jobs::reconcile(state).await.unwrap();
    let cut = jobs::CleanupCommand::DurationExpiry;
    jobs::enqueue(state, id, cut).await.unwrap()
}

/// The key expiry job's pass, for tests with a database of their own.
#[cfg(test)]
pub(crate) async fn key_expiry_pass_for_test(state: &CommsState) {
    jobs::expire_keys(state).await.unwrap();
}

/// Preserve exact native room bindings before startup exposes any scene writers.
/// Batches bound memory; unresolved or noncanonical imported rooms remain NULL.
pub async fn backfill_admin_place_keys(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut after: Option<Uuid> = None;
    loop {
        let rows: Vec<(Uuid, Option<String>)> = sqlx::query_as(
            "SELECT id,room_id FROM social.scene_stream_access WHERE active AND admin_place_key IS NULL AND ($1::uuid IS NULL OR id>$1) ORDER BY id LIMIT 200"
        ).bind(after).fetch_all(pool).await?;
        if rows.is_empty() {
            return Ok(());
        }
        for (id, room) in rows {
            after = Some(id);
            if let Some(room) = room {
                recover_admin_place_key(pool, id, &room).await?;
            }
        }
    }
}

/// Decode with the canonical room codec and recover only an exact live binding.
/// The world/identity locks match publication; recheck the stored room on write.
pub(super) async fn recover_admin_place_key(
    pool: &PgPool,
    id: Uuid,
    room: &str,
) -> Result<Option<String>, sqlx::Error> {
    let Some((world, scene)) = crate::rooms::decode_world_scene_room(room) else {
        return Ok(None);
    };
    let mut tx = pool.begin().await?;
    let world_present: Option<String> =
        sqlx::query_scalar("SELECT name FROM worlds.worlds WHERE name=$1 FOR UPDATE")
            .bind(world.key())
            .fetch_optional(&mut *tx)
            .await?;
    if world_present.is_none() {
        return Ok(None);
    }
    let identities:Vec<(Uuid,Option<String>)>=sqlx::query_as("SELECT identity,entity_id FROM explore.world_scene_place WHERE world_name=$1 AND active ORDER BY identity FOR UPDATE")
        .bind(world.key()).fetch_all(&mut *tx).await?;
    let key = identities
        .into_iter()
        .find(|(_, entity)| entity.as_deref() == Some(scene.as_str()))
        .map(|(identity, _)| identity.to_string());
    let saved = if let Some(key) = &key {
        sqlx::query_scalar("UPDATE social.scene_stream_access SET admin_place_key=$3 WHERE id=$1 AND room_id=$2 AND active AND admin_place_key IS NULL RETURNING admin_place_key")
            .bind(id).bind(room).bind(key).fetch_optional(&mut *tx).await?.flatten()
    } else {
        None
    };
    tx.commit().await?;
    Ok(saved)
}

pub const PATH: &str = "/scene-stream-access";
pub const FOUR_DAYS_MS: i64 = 4 * 24 * 60 * 60 * 1000;
pub const STREAM_ACCESS_NOT_FOUND_MSG: &str = "No active streaming access found for place";
pub const STREAM_ACCESS_EXPIRED_MSG: &str = "Streaming access has expired";
pub const STREAM_RESET_FAILED_MSG: &str = "Failed to reset scene stream access";
pub const NOT_AUTHORIZED_TO_STREAM_MSG: &str = "you are not authorized to stream to this scene";

/// The wire body (upstream's `SceneStreamAccessResponse`), times in ms.
#[derive(Serialize, PartialEq, Eq)]
pub struct SceneStreamAccessResponse {
    pub streaming_url: String,
    pub streaming_key: String,
    pub created_at: i64,
    pub ends_at: i64,
}

impl std::fmt::Debug for SceneStreamAccessResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneStreamAccessResponse")
            .field("streaming_url", &self.streaming_url)
            .field("streaming_key", &"<set>")
            .field("created_at", &self.created_at)
            .field("ends_at", &self.ends_at)
            .finish()
    }
}

#[derive(sqlx::FromRow)]
struct AccessRow {
    id: Uuid,
    room_id: Option<String>,
    streaming: bool,
    streaming_url: String,
    streaming_key: String,
    ingress_id: Option<String>,
    created_at: NaiveDateTime,
    expiration_time: Option<NaiveDateTime>,
    streaming_state_version: i64,
}

impl AccessRow {
    fn response(self) -> SceneStreamAccessResponse {
        let created_ms = self.created_at.and_utc().timestamp_millis();
        let ends_ms = self
            .expiration_time
            .map(|t| t.and_utc().timestamp_millis())
            .unwrap_or(created_ms + FOUR_DAYS_MS);
        SceneStreamAccessResponse {
            streaming_url: self.streaming_url,
            streaming_key: self.streaming_key,
            created_at: created_ms,
            ends_at: ends_ms,
        }
    }
}

const ACCESS_COLUMNS: &str = "id, room_id, streaming, streaming_url, streaming_key, ingress_id, created_at, expiration_time, streaming_state_version";

pub async fn scene_stream_access(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if !state.livekit.configured {
        return Err(service_unavailable(
            "LiveKit ingress is not configured (set LIVEKIT_API_KEY / LIVEKIT_API_SECRET)",
        ));
    }
    let verb = method.as_str().to_ascii_lowercase();
    let sf = verify_signed_fetch(
        &headers,
        &verb,
        &SignedPath::at(&uri, PATH),
        &[SCENE_SIGNER],
        state.eip1654(),
    )
    .await?;
    realm_name_from_metadata(&sf.metadata, &state.settings.realm_name)
        .ok_or_else(|| ApiError::bad_request("invalid signed-fetch request, no realmName"))?;
    let device_id = device_identifier(&sf.metadata);
    if method != Method::DELETE {
        let denied = forbidden(NOT_AUTHORIZED_TO_STREAM_MSG);
        refuse_barred(&state, &sf.signer, device_id.as_deref(), denied).await?;
    }
    let (place_id, place, room, scene, admin_place_key) = if method == Method::DELETE {
        // Revocation needs no current deployment: the place is found by the
        // signed realm and parcel alone, and the authority on that same realm.
        let realm = Context::signed_realm(&sf.metadata, &state.settings.realm_name)?;
        let world = realm.world();
        let parcel = meta_str(&sf.metadata, "parcel");
        if world.is_none() && parcel.is_none() {
            return Err(ApiError::bad_request(
                "invalid signed-fetch request, no parcel",
            ));
        }
        let place_id = resolve_place_id(
            &state.pool,
            world,
            parcel.as_deref().unwrap_or_default(),
            false,
        )
        .await?
        .ok_or_else(|| ApiError::not_found("place not found for this scene"))?;
        let place =
            revocation_authority(&state, &sf.metadata, &realm, parcel.as_deref(), &place_id)
                .await?;
        // In a world, the signed scene is resolved only if the world-wide
        // decision refuses (see below), so a revocation by the owner never
        // waits on the resolver.
        let scene = match realm.world() {
            Some(_) => KeyScene::Signed,
            None => KeyScene::None,
        };
        (place_id, place, None, scene, String::new())
    } else {
        // One resolution binds the signed scene to its parcel; the room and the
        // authorizing footprint both come from it.
        let bound = Context::resolve(&state, &sf.metadata).await?;
        let world = bound.world_scene.as_ref().map(|(world, _)| world);
        let base = bound.footprint.base_position.clone().unwrap_or_default();
        let place_id = resolve_place_id(&state.pool, world, &base, true)
            .await?
            .ok_or_else(|| ApiError::not_found("place not found for this scene"))?;
        // The credential is stored under the catalogue place; the scene's
        // explicit admins are granted on its place (`/scene-admin`), which
        // `bound.place` carries with the deployment's footprint.
        let scene = match world {
            Some(_) => KeyScene::Bound(bound.place.admin_key().to_owned()),
            None => KeyScene::None,
        };
        let admin_place_key = bound.place.admin_key().to_owned();
        let place = key_authority(&state, &place_id, world, bound.place);
        (place_id, place, Some(bound.room), scene, admin_place_key)
    };
    let mut decision = scene::scene_manager_decision(&state, &place, &sf.signer).await;
    if lacks_authority(&decision) {
        // A world's scene admin may revoke the key it may reset: the signed
        // scene when it is the world's current deployment.
        let world_scene = match scene {
            KeyScene::None => None,
            KeyScene::Bound(key) => Some(key),
            KeyScene::Signed => match Context::resolve(&state, &sf.metadata).await {
                Ok(bound) => Some(bound.place.admin_key().to_owned()),
                Err(error) if error.code >= 500 => return Err(error),
                Err(_) => None,
            },
        };
        if let Some(world_scene) = world_scene {
            decision = world_scene_admin(&state, &place, &world_scene, &sf.signer, decision).await;
        }
    }
    match decision {
        PermissionDecision::Allowed => {}
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
            return Err(forbidden(NOT_AUTHORIZED_TO_STREAM_MSG));
        }
        PermissionDecision::Refused(why) => return Err(why.into()),
    }
    let room = room.unwrap_or_default();
    let ingress = state.ingress();
    if method == Method::GET {
        // A key the place no longer holds (its scene redeployed, its minter
        // without the place) is never the one answered.
        cast::recheck_keys(&state, cast::Keys::Place(&place_id)).await;
    }
    match method {
        Method::GET => match active_access(&state.pool, &place_id).await? {
            Some(row) => {
                // Upstream's list handler: the stored expiry is the one
                // answered, and a credential past it that the expiry job has
                // not retired yet is already gone.
                let resp = row.response();
                if resp.ends_at <= chrono::Utc::now().timestamp_millis() {
                    return Err(ApiError::not_found(STREAM_ACCESS_EXPIRED_MSG));
                }
                Ok(Json(resp).into_response())
            }
            None => Err(ApiError::not_found(STREAM_ACCESS_NOT_FOUND_MSG)),
        },
        Method::POST => issue_access(
            &state,
            &ingress,
            &place_id,
            &room,
            &sf.signer,
            &admin_place_key,
        )
        .await
        .map(|resp| Json(resp).into_response()),
        Method::PUT => reset_access(
            &state,
            &ingress,
            &place_id,
            &room,
            &sf.signer,
            &admin_place_key,
        )
        .await
        .map(|resp| Json(resp).into_response()),
        Method::DELETE => remove_access(&state, &ingress, &place_id).await,
        _ => Ok(StatusCode::METHOD_NOT_ALLOWED.into_response()),
    }
}

/// The lookup an admin decision runs on for the scene behind `resolved`:
/// `/scene-admin` grants a scene's explicit admins on its place
/// (`ActiveDeployment::place_key`), never on the catalogue place a stream
/// credential is stored for. The footprint
/// that decides ownership is the active deployment's, or `stored_place`'s row
/// when the room names a scene this node did not resolve a deployment for.
/// `None` when neither is known: the caller then asks the stored place alone,
/// which admits its owner only. A catalogue that cannot be read is an error,
/// never that narrower lookup.
pub(super) async fn scene_authority<'a>(
    state: &'a CommsState,
    resolved: &ResolvedRoom,
    stored_place: &str,
) -> Result<Option<PlaceLookup<'a>>, ApiError> {
    if let Some(deployment) = resolved.deployment.as_ref() {
        let info = scene::PlaceInfo {
            world: resolved.realm.is_world(),
            world_name: resolved.realm.world().map(|world| world.key().to_owned()),
            positions: deployment.parcels.iter().map(Parcel::canonical).collect(),
            base_position: Some(deployment.base.canonical()),
        };
        let key = deployment.place_key();
        let scene = deployment.scene_id.as_str();
        return Ok(Some(PlaceLookup::resolved(state, scene, info, &key)));
    }
    let Some(scene) = resolved.scene_id.as_ref() else {
        return Ok(None);
    };
    let info = scene::try_load_place_info(&state.pool, stored_place)
        .await
        .map_err(|error| {
            tracing::error!(%error, stored_place, "place lookup failed for a scene authority");
            service_unavailable(scene::PLACE_LOOKUP_UNAVAILABLE_MSG)
        })?;
    // A Genesis room names its scene alone: the credential's place is based
    // on the scene's base parcel, the place key.
    Ok(info.map(|info| {
        let key = info.base_position.clone().filter(|_| !info.world);
        let key = key.as_deref().unwrap_or(scene::NO_ADMINS);
        PlaceLookup::resolved(state, scene.as_str(), info, key)
    }))
}

/// The lookup the stream-key verbs decide on. A world has ONE credential for
/// all its scenes, so authority over one scene (a grantee of some of its
/// parcels) must not reach it: the world place with no footprint admits the
/// owner and a grantee whose grant has no parcel restriction, and no
/// `/scene-admin` grant. A Genesis place is its scene's. The one scene-level grant
/// that reaches a world's key is decided apart, in [`world_scene_admin`].
pub(super) fn key_authority<'a>(
    state: &'a CommsState,
    place_id: &str,
    world: Option<&WorldName>,
    scene: PlaceLookup<'a>,
) -> PlaceLookup<'a> {
    match world {
        Some(world) => world_authority(state, place_id, world),
        None => scene,
    }
}

/// The world place with no footprint: its owner and the grantees with no
/// parcel restriction hold it, and no `/scene-admin` grant does.
pub(super) fn world_authority<'a>(
    state: &'a CommsState,
    place_id: &str,
    world: &WorldName,
) -> PlaceLookup<'a> {
    PlaceLookup::resolved(
        state,
        place_id,
        scene::PlaceInfo {
            world: true,
            world_name: Some(world.key().to_owned()),
            positions: Vec::new(),
            base_position: None,
        },
        scene::NO_ADMINS,
    )
}

/// The decision for a stream-key verb on `authority` ([`key_authority`]),
/// with the admin key of the world scene the request is bound to, if any.
pub(super) async fn key_decision(
    state: &CommsState,
    authority: &PlaceLookup<'_>,
    world_scene: Option<&str>,
    signer: &Address,
) -> PermissionDecision {
    let decision = scene::scene_manager_decision(state, authority, signer).await;
    match world_scene {
        Some(scene) if lacks_authority(&decision) => {
            world_scene_admin(state, authority, scene, signer, decision).await
        }
        _ => decision,
    }
}

pub(super) fn lacks_authority(decision: &PermissionDecision) -> bool {
    matches!(
        decision,
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. })
    )
}

/// After the world-wide decision `refused` the signer for lack of authority:
/// an explicit admin of the world `scene` holds the world's one credential
/// when whoever granted it still holds world-wide authority. `/scene-admin`
/// keys that grant on the scene's place (`scene`), so no admin of the world
/// place can be granted, and upstream's admin of a world's place holds its stream
/// key. A grantee restricted to some parcels may grant `/scene-admin` on its
/// scene, so an admin it granted (or one granted by another scene admin)
/// stays refused: the grant never widens its granter's authority.
async fn world_scene_admin(
    state: &CommsState,
    authority: &PlaceLookup<'_>,
    scene: &str,
    signer: &Address,
    refused: PermissionDecision,
) -> PermissionDecision {
    let granter = match state.moderation.active_admin_granter(scene, signer).await {
        Ok(Some(granter)) => granter,
        Ok(None) => return refused,
        Err(error) => return PermissionDecision::undetermined("social.scene_admins", &error),
    };
    match scene::scene_manager_decision(state, authority, &granter).await {
        PermissionDecision::Allowed => PermissionDecision::Allowed,
        decision if lacks_authority(&decision) => refused,
        undetermined => undetermined,
    }
}

/// The world scene a stream-key verb may fall back to ([`world_scene_admin`]).
enum KeyScene {
    None,
    /// The admin key of the scene the request is bound to.
    Bound(String),
    /// A revocation: the signed scene, resolved only when needed.
    Signed,
}

/// The lookup a revocation is authorized on. A world's credential takes
/// authority over the whole world ([`key_authority`]). A Genesis place: the
/// signed scene when it is the current deployment, as for the other verbs;
/// else the scene standing on the signed parcel now, so the admins of the
/// scene that replaced it can still revoke the key; with no scene there, the
/// place's owner alone. A resolver or catalogue that cannot be read is an
/// error (503), never one of the narrower lookups.
async fn revocation_authority<'a>(
    state: &'a CommsState,
    metadata: &serde_json::Value,
    realm: &RealmId,
    parcel: Option<&str>,
    place_id: &str,
) -> Result<PlaceLookup<'a>, ApiError> {
    let stored = PlaceLookup::new(state, place_id, scene::NO_ADMINS);
    if let Some(world) = realm.world() {
        return Ok(key_authority(state, place_id, Some(world), stored));
    }
    match Context::resolve(state, metadata).await {
        Ok(bound) => return Ok(bound.place),
        Err(error) if error.code >= 500 => return Err(error),
        // Not the current deployment (or not a bindable scene): the parcel decides.
        Err(_) => {}
    }
    let Some(parcel) = parcel.and_then(|raw| Parcel::parse(raw).ok()) else {
        return Ok(stored);
    };
    let current = match state
        .scenes
        .resolve(RoomRequest::Parcel { realm, parcel })
        .await
    {
        Ok(current) => current,
        Err(crate::rooms::ResolveError::NoSceneAtParcel { .. }) => return Ok(stored),
        Err(error) => {
            tracing::warn!(%error, place_id, "scene resolver unavailable for a stream key revocation");
            return Err(service_unavailable("Scene identity is unavailable"));
        }
    };
    Ok(scene_authority(state, &current, place_id)
        .await?
        .unwrap_or(stored))
}

/// The place's active credential when it is still valid for `room`, else a
/// new one: an idle credential for another room or past its expiry is
/// replaced (its ingress cleanup is a durable intent), a streaming one is a
/// 409 unless LiveKit reports its ingress is not publishing (a lost
/// `ingress_ended` webhook), and so is one LiveKit reports publishing with
/// the flag down (a lost `ingress_started`). `POST /scene-stream-access` and
/// `/cast/generate-stream-link` share it.
async fn issue_access(
    state: &SharedState,
    ingress: &IngressClient<'_>,
    place_id: &str,
    room: &str,
    signer: &Address,
    admin_place_key: &str,
) -> Result<SceneStreamAccessResponse, ApiError> {
    // A key whose minter lost the place is never the one answered.
    cast::recheck_keys(state, cast::Keys::Place(place_id)).await;
    if let Some(existing) = active_access(&state.pool, place_id).await? {
        match renewal(&existing, room, now_utc()) {
            Renewal::Reuse => return Ok(existing.response()),
            Renewal::Conflict => {
                if !clear_stale_streaming(state, ingress, &existing, place_id).await? {
                    return Err(renewal_conflict());
                }
            }
            Renewal::Replace => {
                if raise_lost_start(state, ingress, &existing).await? {
                    return Err(renewal_conflict());
                }
            }
        }
        check_generation_quota(&state.pool, place_id).await?;
        // The replacement's ingress first: a LiveKit failure keeps the old credential.
        let info = mint_ingress(ingress, room).await?;
        return replace_access(
            state,
            ingress,
            existing.id,
            jobs::CleanupCommand::IdleReplacement,
            (place_id, room, signer, admin_place_key),
            info,
        )
        .await;
    }
    create_access(
        &state.pool,
        ingress,
        place_id,
        room,
        signer,
        admin_place_key,
    )
    .await
}

/// Clears `row`'s `streaming` flag when LiveKit says its ingress is gone or
/// not publishing, so a lost `ingress_ended` webhook cannot block the place's
/// renewal. Only the RTMP ingress sets the flag (Cast WebRTC publishers never
/// do). False keeps the 409: the ingress is live, its state is unknown,
/// LiveKit is unreachable, or the row changed meanwhile.
async fn clear_stale_streaming(
    state: &CommsState,
    ingress: &IngressClient<'_>,
    row: &AccessRow,
    place_id: &str,
) -> Result<bool, ApiError> {
    let ingress_id = row.ingress_id.as_deref().unwrap_or_default();
    if !ingress_id.is_empty() {
        match ingress.is_ingress_streaming(ingress_id).await {
            Ok(Some(false)) => {}
            Ok(_) => return Ok(false),
            Err(error) => {
                tracing::warn!(place_id, access_id = %row.id, %error, "ingress state unavailable; renewal refused");
                return Ok(false);
            }
        }
    }
    let cleared = sqlx::query(
        "UPDATE social.scene_stream_access SET streaming = false, streaming_start_time = NULL \
         WHERE id = $1 AND active AND streaming AND streaming_state_version = $2",
    )
    .bind(row.id)
    .bind(row.streaming_state_version)
    .execute(&state.pool)
    .await?
    .rows_affected()
        == 1;
    if cleared {
        tracing::info!(place_id, access_id = %row.id, ingress_id, "cleared a stale streaming flag: the ingress is not publishing");
    }
    Ok(cleared)
}

/// Raises `row`'s `streaming` flag when it is down and LiveKit says its
/// ingress publishes (a lost `ingress_started` webhook), so the broadcast is
/// not replaced. False leaves the replacement to go on: the ingress is not
/// live, or its state is unknown.
async fn raise_lost_start(
    state: &CommsState,
    ingress: &IngressClient<'_>,
    row: &AccessRow,
) -> Result<bool, ApiError> {
    let ingress_id = row.ingress_id.as_deref().unwrap_or_default();
    if ingress_id.is_empty()
        || !matches!(
            ingress.is_ingress_streaming(ingress_id).await,
            Ok(Some(true))
        )
    {
        return Ok(false);
    }
    jobs::raise(&state.pool, row.id, row.streaming_state_version).await?;
    Ok(true)
}

/// Why `wallet` may use no stream key and no Cast route, if so: the platform
/// banned it (or its device), or the deny list holds it. The one gate of the
/// routes, of the minter of a key and of a watcher that joins.
pub(super) async fn barred(
    state: &CommsState,
    wallet: &Address,
    device_id: Option<&str>,
) -> Result<Option<&'static str>, ApiError> {
    let now = chrono::Utc::now().naive_utc();
    let (policy, bans) = (state.policy.as_ref(), state.bans.as_ref());
    let ban =
        crate::gatekeeper::moderation::admission_ban(policy, bans, wallet, None, device_id, now);
    Ok(match ban.await? {
        Some(_) => Some(BANNED),
        None if state.deny_list.is_denied(wallet.as_str()).await => Some("on the deny list"),
        None => None,
    })
}
const BANNED: &str = "banned from the platform";

/// Refuses a [`barred`] wallet: 403 for a platform ban, `denied` for the deny list.
async fn refuse_barred(
    state: &CommsState,
    signer: &Address,
    device_id: Option<&str>,
    denied: ApiError,
) -> Result<(), ApiError> {
    match barred(state, signer, device_id).await? {
        None => Ok(()),
        Some(BANNED) => {
            tracing::warn!(address = %signer, "rejected a stream key request from a platform-banned user");
            Err(forbidden(PLATFORM_BANNED_MSG))
        }
        Some(_) => Err(denied),
    }
}

/// The place the bound scene stands on: its world (published here or
/// mirrored) by name, else the Genesis place holding `parcel`. With
/// `based_there` only a place based on `parcel` counts, so a catalogue that
/// has not caught up with a redeploy never lends another scene's admins.
async fn resolve_place_id(
    pool: &PgPool,
    world: Option<&WorldName>,
    parcel: &str,
    based_there: bool,
) -> Result<Option<String>, ApiError> {
    if let Some(world) = world {
        return Ok(sqlx::query_scalar(
            "SELECT id FROM explore.place WHERE world AND lower(world_name) = lower($1) \
             UNION ALL \
             SELECT id FROM explore.place_world_local WHERE world AND lower(world_name) = lower($1) \
             LIMIT 1",
        )
        .bind(world.display())
        .fetch_optional(pool)
        .await?);
    }
    // A live Genesis place on the parcel, preferring the one based there.
    Ok(sqlx::query_scalar(
        "SELECT id FROM explore.place WHERE NOT world AND NOT disabled \
         AND raw->'positions' @> jsonb_build_array($1::text) \
         AND (NOT $2 OR base_position = $1) \
         ORDER BY base_position = $1 DESC, deployed_at DESC NULLS LAST, id LIMIT 1",
    )
    .bind(parcel)
    .bind(based_there)
    .fetch_optional(pool)
    .await?)
}

async fn active_access(pool: &PgPool, place_id: &str) -> Result<Option<AccessRow>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ACCESS_COLUMNS} FROM social.scene_stream_access WHERE place_id = $1 AND active"
    )))
    .bind(place_id)
    .fetch_optional(pool)
    .await
}

/// Mints (or reuses) the room's ingress and records it as the place's
/// active credential. A concurrent mint that recorded first wins: its row is
/// answered, and an ingress this call created for nothing is deleted.
async fn create_access(
    pool: &PgPool,
    ingress: &IngressClient<'_>,
    place_id: &str,
    room: &str,
    signer: &Address,
    admin_place_key: &str,
) -> Result<SceneStreamAccessResponse, ApiError> {
    check_generation_quota(pool, place_id).await?;
    let info = mint_ingress(ingress, room).await?;
    store_access(pool, ingress, place_id, room, signer, admin_place_key, info).await
}

/// The new credential's RTMP ingress. Minted before an old credential is
/// revoked (as upstream does), so a LiveKit failure leaves the old one in place.
async fn mint_ingress(
    ingress: &IngressClient<'_>,
    room: &str,
) -> Result<crate::shared::livekit::IngressInfo, ApiError> {
    let participant_identity = format!("{}-streamer", Uuid::new_v4());
    let info = ingress
        .create_ingress(room, &participant_identity)
        .await
        .map_err(|e| ApiError::internal(format!("livekit ingress create: {e}")))?;
    if info.url.is_none() || info.stream_key.is_none() {
        return Err(ApiError::internal(
            "livekit ingress response missing url or stream key",
        ));
    }
    Ok(info)
}

/// Replaces credential `old` with one for the freshly minted `info`: the old
/// row's deactivation, its cleanup intent and the new row are one
/// transaction, so any failure (a concurrent change included) leaves the old
/// credential in force and deletes the new ingress. For a reset that trades
/// security for availability: a reset that answers 500 or 409 leaves the key
/// it meant to replace working, and the admin retries it (or revokes).
async fn replace_access(
    state: &SharedState,
    ingress: &IngressClient<'_>,
    old: Uuid,
    command: jobs::CleanupCommand,
    (place_id, room, signer, admin_place_key): (&str, &str, &Address, &str),
    info: crate::shared::livekit::IngressInfo,
) -> Result<SceneStreamAccessResponse, ApiError> {
    let stored = async {
        let Some(prepared) = jobs::prepare(state, old, command).await? else {
            return Ok(None);
        };
        let mut tx = state.pool.begin().await?;
        lock_generation_place(&mut tx, place_id).await?;
        if !prepared.record(&mut *tx).await? {
            return Ok(None);
        }
        let Some(row) =
            insert_access(&mut tx, place_id, room, signer, admin_place_key, &info).await?
        else {
            return Ok(None);
        };
        tx.commit().await?;
        Ok::<_, ApiError>(Some(row))
    }
    .await;
    match stored {
        Ok(Some(row)) => {
            settle_cleanup(state, old, place_id);
            Ok(row.response())
        }
        Ok(None) => {
            discard_ingress(ingress, &info, place_id).await;
            Err(concurrent_change())
        }
        Err(error) => {
            discard_ingress(ingress, &info, place_id).await;
            Err(error)
        }
    }
}

/// Ends a removed place's credential in the removal's transaction, as a
/// revocation does (its cleanup deletes the ingress and removes the Cast
/// publishers) but with no key notice: the removal tells the owner. Returns
/// the credential to settle after the commit.
pub(crate) async fn end_removed_place(
    tx: &mut sqlx::PgConnection,
    place_id: &str,
    removed_at: NaiveDateTime,
) -> Result<Option<Uuid>, sqlx::Error> {
    // A key made after the removal belongs to the place that came back.
    let row: Option<(Uuid, i64)> = sqlx::query_as(
        "SELECT id, streaming_state_version FROM social.scene_stream_access \
         WHERE place_id = $1 AND active AND created_at <= $2 FOR UPDATE",
    )
    .bind(place_id)
    .bind(removed_at)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((id, version)) = row else {
        return Ok(None);
    };
    let command = jobs::CleanupCommand::PlaceRemoval;
    jobs::enqueue_prepared(tx, id, command, version, None, None, None).await?;
    Ok(Some(id))
}

/// Runs the committed cleanup now, in a task of its own: the swap or
/// revocation is already stored and the key refused, so no caller waits for
/// LiveKit. Its failure is logged, and the durable job retries it.
pub(crate) fn settle_cleanup(state: &SharedState, id: Uuid, place_id: &str) {
    let (state, place_id) = (state.clone(), place_id.to_owned());
    crate::shared::task::spawn(async move {
        if let Err(error) = jobs::cleanup_access(&state, id).await {
            tracing::warn!(place_id, access_id = %id, error = %error, "stream cleanup deferred to the job");
        }
    });
}

/// Deletes an ingress minted for a credential that was not stored.
async fn discard_ingress(
    ingress: &IngressClient<'_>,
    info: &crate::shared::livekit::IngressInfo,
    place_id: &str,
) {
    if !info.ingress_id.is_empty()
        && let Err(e) = ingress.delete_ingress(&info.ingress_id).await
    {
        tracing::warn!(error = %e, place_id, "failed to delete an unused ingress");
    }
}

/// Every issuance takes this lock before touching the active credential, so
/// concurrent reset/create requests observe committed per-place quota facts.
async fn lock_generation_place(
    tx: &mut sqlx::PgConnection,
    place_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,1571))")
        .bind(place_id)
        .execute(tx)
        .await?;
    Ok(())
}

/// The pre-mint check avoids known quota refusals doing external work. Insertion
/// repeats this check under the per-place lock to enforce the bound against races.
async fn check_generation_quota<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
    place_id: &str,
) -> Result<(), ApiError> {
    let recent: i64 = sqlx::query_scalar("SELECT count(*) FROM (SELECT 1 FROM social.scene_stream_access WHERE place_id=$1 AND created_at>timezone('UTC',statement_timestamp())-interval '1 hour' LIMIT 6) recent")
        .bind(place_id).fetch_one(executor).await?;
    if recent >= 6 {
        return Err(ApiError::http(
            429,
            "Stream keys can be generated at most six times per hour for this place. Please try again later.",
        ));
    }
    Ok(())
}

/// Inserts the place's active credential for `info`'s ingress; `None` when
/// the place already has one.
async fn insert_access(
    executor: &mut sqlx::PgConnection,
    place_id: &str,
    room: &str,
    signer: &Address,
    admin_place_key: &str,
    info: &crate::shared::livekit::IngressInfo,
) -> Result<Option<AccessRow>, ApiError> {
    let (Some(url), Some(stream_key)) = (info.url.as_deref(), info.stream_key.as_deref()) else {
        return Err(ApiError::internal(
            "livekit ingress response missing url or stream key",
        ));
    };
    lock_generation_place(executor, place_id).await?;
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM social.scene_stream_access WHERE place_id=$1 AND active)",
    )
    .bind(place_id)
    .fetch_one(&mut *executor)
    .await?;
    if active {
        return Ok(None);
    }
    check_generation_quota(&mut *executor, place_id).await?;
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO social.scene_stream_access \
             (place_id, streaming_url, streaming_key, ingress_id, room_id, generated_by, created_at, expiration_time, active, admin_place_key) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $7 + make_interval(secs => $8::double precision / 1000), true, $9) \
         ON CONFLICT (place_id) WHERE active DO NOTHING \
         RETURNING {ACCESS_COLUMNS}"
    )))
    .bind(place_id)
    .bind(url)
    .bind(stream_key)
    .bind(&info.ingress_id)
    .bind(room)
    .bind(signer)
    .bind(now_utc())
    .bind(FOUR_DAYS_MS)
    .bind(admin_place_key)
    .fetch_optional(executor)
    .await?)
}

async fn store_access(
    pool: &PgPool,
    ingress: &IngressClient<'_>,
    place_id: &str,
    room: &str,
    signer: &Address,
    admin_place_key: &str,
    info: crate::shared::livekit::IngressInfo,
) -> Result<SceneStreamAccessResponse, ApiError> {
    let stored = async {
        let mut tx = pool.begin().await?;
        let inserted =
            insert_access(&mut tx, place_id, room, signer, admin_place_key, &info).await?;
        tx.commit().await?;
        Ok::<_, ApiError>(inserted)
    }
    .await;
    let inserted = match stored {
        Ok(inserted) => inserted,
        Err(error) => {
            discard_ingress(ingress, &info, place_id).await;
            return Err(error);
        }
    };
    if let Some(row) = inserted {
        return Ok(row.response());
    }
    let winner = active_access(pool, place_id).await?.ok_or_else(|| {
        ApiError::internal("scene stream access vanished during a concurrent mint")
    })?;
    if !info.ingress_id.is_empty()
        && winner.ingress_id.as_deref() != Some(info.ingress_id.as_str())
        && let Err(e) = ingress.delete_ingress(&info.ingress_id).await
    {
        tracing::warn!(error = %e, place_id, "failed to delete an ingress that lost a concurrent mint");
    }
    if renewal(&winner, room, now_utc()) == Renewal::Reuse {
        Ok(winner.response())
    } else {
        Err(renewal_conflict())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Renewal {
    Reuse,
    Replace,
    Conflict,
}

fn renewal(row: &AccessRow, room: &str, now: NaiveDateTime) -> Renewal {
    let expiry = row
        .expiration_time
        .unwrap_or(row.created_at + chrono::Duration::days(4));
    if row.room_id.as_deref() == Some(room) && expiry > now {
        Renewal::Reuse
    } else if row.streaming {
        Renewal::Conflict
    } else {
        Renewal::Replace
    }
}

/// Upstream's `StreamRenewalConflictError`.
pub const RENEWAL_CONFLICT_MSG: &str = "A broadcast is still live. Stop it before renewing, or explicitly reset the stream key to interrupt it.";

fn renewal_conflict() -> ApiError {
    ApiError::http(409, RENEWAL_CONFLICT_MSG)
}

/// The credential was replaced, revoked or touched by a webhook while this
/// request worked on it: nothing need be streaming, and a retry decides anew.
pub const CONCURRENT_CHANGE_MSG: &str =
    "The stream access changed while this request was handling it. Please try again.";

fn concurrent_change() -> ApiError {
    ApiError::http(409, CONCURRENT_CHANGE_MSG)
}

async fn reset_access(
    state: &SharedState,
    ingress: &IngressClient<'_>,
    place_id: &str,
    room: &str,
    signer: &Address,
    admin_place_key: &str,
) -> Result<SceneStreamAccessResponse, ApiError> {
    let result = async {
        let row = active_access(&state.pool, place_id)
            .await?
            .ok_or_else(|| ApiError::not_found(STREAM_ACCESS_NOT_FOUND_MSG))?;
        check_generation_quota(&state.pool, place_id).await?;
        // The replacement's ingress first: a LiveKit failure keeps the old credential.
        // The durable intent owns the old ingress's deletion and the reset notification,
        // delivered only once the replacement credential exists; it also covers a crash.
        let info = mint_ingress(ingress, room).await?;
        replace_access(
            state,
            ingress,
            row.id,
            jobs::CleanupCommand::Reset,
            (place_id, room, signer, admin_place_key),
            info,
        )
        .await
    }
    .await;
    result.map_err(|error: ApiError| {
        // Preserve not-found and issuance-limit responses; swap failures use the reset error.
        if error.code == 404 || error.code == 429 {
            return error;
        }
        tracing::error!(place_id, error = %error, "scene stream access reset failed");
        ApiError::http(500, STREAM_RESET_FAILED_MSG)
    })
}

async fn remove_access(
    state: &SharedState,
    _ingress: &IngressClient<'_>,
    place_id: &str,
) -> Result<Response, ApiError> {
    let row = active_access(&state.pool, place_id)
        .await?
        .ok_or_else(|| ApiError::not_found("no active stream access for this scene"))?;
    if !jobs::enqueue(state, row.id, jobs::CleanupCommand::Revoke).await? {
        return Err(concurrent_change());
    }
    settle_cleanup(state, row.id, place_id);
    Ok(StatusCode::NO_CONTENT.into_response())
}

pub(super) enum IngressEvent {
    Started,
    Ended,
}

/// The LiveKit webhook's ingress events: the active row of that ingress
/// starts streaming (keeping the first start time) or stops. The events are
/// not ordered, so an `ingress_ended` stops it only when LiveKit does not
/// report the ingress live now: one from before the latest start is ignored.
pub(super) async fn ingress_event(
    state: &CommsState,
    event: IngressEvent,
    ingress_id: Option<&str>,
) -> Result<(), ApiError> {
    let Some(ingress_id) = ingress_id.filter(|id| !id.is_empty()) else {
        return Ok(());
    };
    match event {
        IngressEvent::Started => {
            sqlx::query(
                "UPDATE social.scene_stream_access SET streaming = true, streaming_start_time = CASE WHEN streaming THEN COALESCE(streaming_start_time,$2) ELSE $2 END, streaming_state_version=streaming_state_version+1 \
                 WHERE ingress_id = $1 AND active",
            )
            .bind(ingress_id)
            .bind(now_utc())
            .execute(&state.pool)
            .await?;
        }
        IngressEvent::Ended => {
            let live = state.ingress().is_ingress_streaming(ingress_id).await;
            if matches!(live, Ok(Some(true))) {
                return Ok(());
            }
            sqlx::query("UPDATE social.scene_stream_access SET streaming = false, streaming_state_version=streaming_state_version+1 WHERE ingress_id = $1 AND active").bind(ingress_id).execute(&state.pool).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;
    use crate::gatekeeper::{Settings, router};
    use crate::http::Outbound;
    use crate::interfaces::test_support::{StubPolicy, StubScenes};

    #[test]
    fn the_access_body_is_upstreams_shape_and_hides_the_key_from_debug() {
        let body = SceneStreamAccessResponse {
            streaming_url: "rtmp://ingest/x".into(),
            streaming_key: "sk_abc".into(),
            created_at: 1_700_000_000_000,
            ends_at: 1_700_345_600_000,
        };
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::json!({ "streaming_url": "rtmp://ingest/x", "streaming_key": "sk_abc", "created_at": 1_700_000_000_000i64, "ends_at": 1_700_345_600_000i64 })
        );
        assert!(!format!("{body:?}").contains("sk_abc"));
        assert_eq!(FOUR_DAYS_MS, 345_600_000);
    }

    #[test]
    fn renewal_preserves_valid_keys_and_refuses_only_conflicting_streams() {
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .unwrap()
            .naive_utc();
        let mut row = AccessRow {
            id: Uuid::nil(),
            room_id: Some("room".into()),
            streaming: true,
            streaming_url: "rtmp://example.org".into(),
            streaming_key: "key".into(),
            ingress_id: None,
            created_at: now,
            expiration_time: None,
            streaming_state_version: 0,
        };
        assert_eq!(renewal(&row, "room", now), Renewal::Reuse);
        assert_eq!(renewal(&row, "new-room", now), Renewal::Conflict);
        assert_eq!(
            renewal(&row, "room", now + chrono::Duration::days(4)),
            Renewal::Conflict
        );
        row.streaming = false;
        assert_eq!(renewal(&row, "new-room", now), Renewal::Replace);
        assert_eq!(
            renewal(&row, "room", now + chrono::Duration::days(4)),
            Renewal::Replace
        );
    }

    #[tokio::test]
    async fn cast_refuses_an_unsigned_caller_and_an_unconfigured_livekit_answers_503() {
        let pool = PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
        let unconfigured = Settings {
            livekit: super::super::LiveKit {
                configured: false,
                ..Settings::for_tests().livekit
            },
            ..Settings::for_tests()
        };
        let app = router(Arc::new(CommsState::new(
            pool,
            Outbound::for_tests(),
            unconfigured,
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(crate::interfaces::test_support::StubDirectory::default()),
        )));
        let resp = app
            .clone()
            .oneshot(
                Request::get("/cast/presenters")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // The signature gate answers before the LiveKit configuration is consulted.
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let resp = app
            .oneshot(Request::get(PATH).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}

#[cfg(test)]
mod moderation_boundary_tests {
    use super::*;
    use crate::gatekeeper::moderation::test_support::{StubBanStatus, state};
    use crate::interfaces::test_support::StubPolicy;
    #[tokio::test]
    async fn operator_block_refuses_streaming_without_ingress_or_permission_io() {
        let state = state(
            StubPolicy {
                blocked: Ok(true),
                ..Default::default()
            },
            StubBanStatus(Err(crate::AuthorityNotEstablished::store_unavailable(
                "unexpected raw read",
                &sqlx::Error::PoolClosed,
            ))),
        );
        let wallet = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        assert_eq!(
            refuse_barred(&state, &wallet, None, forbidden(""))
                .await
                .unwrap_err()
                .code,
            403
        );
    }
    #[tokio::test]
    async fn block_authority_fault_keeps_streaming_unavailable() {
        let state = state(
            StubPolicy {
                blocked: Err(crate::AuthorityNotEstablished::store_unavailable(
                    "worlds.blocked_wallets",
                    &sqlx::Error::PoolClosed,
                )),
                ..Default::default()
            },
            StubBanStatus::default(),
        );
        let wallet = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        assert_eq!(
            refuse_barred(&state, &wallet, None, forbidden(""))
                .await
                .unwrap_err()
                .code,
            503
        );
    }
}
