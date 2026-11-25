//! `/scene-stream-access` (GET/POST/PUT/DELETE, scene signed): the RTMP
//! ingress credential of a place's stream. The signer must own or
//! administer the place the signed metadata stands on. List, add and reset
//! require the signed scene to be the current deployment on its parcel and
//! authorize against that deployment: its footprint decides ownership and its
//! place key finds the explicit admins `/scene-admin` granted. Each scene has
//! its own credential, streaming into its own room: a Genesis scene's is
//! stored for the catalogue place based on its base parcel, a world scene's
//! for its place (`ActiveDeployment::place_key`), as upstream keys it on the
//! world scene's place ([`KeyTarget`]). Remove also takes the scene standing
//! on the signed parcel now, and every verb first rechecks the place's keys,
//! so the key of a scene a redeploy replaced is revoked by the admins of the
//! scene standing there now. A world scene lists, adds and resets only while
//! its world has a catalogue place. List, add and reset are refused to
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
/// For the Cast presenter, which replaces a room's presentation only for a
/// bot of the room's active credential.
pub(crate) use cast::publisher_credential_current;
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

/// Decode with the canonical room codec and recover only an exact live binding
/// that is unique across current and retained bindings.
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
    // As deferred recovery does: a retained binding of another identity for
    // the same scene makes the live one ambiguous, so nothing is saved.
    let unique: Option<String> = sqlx::query_scalar(
        "WITH b AS (SELECT identity FROM explore.world_scene_binding WHERE world_name=$1 AND entity_id=$2 \
         UNION SELECT identity FROM explore.world_scene_place WHERE world_name=$1 AND entity_id=$2 AND active) \
         SELECT min(identity::text) FROM b HAVING count(*)=1",
    )
    .bind(world.key())
    .bind(scene.as_str())
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    let key = key.filter(|key| unique.as_deref() == Some(key.as_str()));
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

/// Which of several active keys streaming into one world scene room is the
/// room's (a mixed rollout can leave two): the one stored under its own
/// scene's place, then the newest. Every reader by room shares it, so the
/// verbs and the presenter routes act on the same key.
pub(super) const ROOM_KEY_ORDER: &str =
    "(place_id = admin_place_key) IS TRUE DESC, created_at DESC, id DESC";

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
    let target = if method == Method::DELETE {
        revocation_target(&state, &sf.metadata).await?
    } else {
        // One resolution binds the signed scene to its parcel; the room and the
        // authorizing footprint both come from it.
        let bound = Context::resolve(&state, &sf.metadata).await?;
        if let Some((world, _)) = &bound.world_scene {
            require_world_place(&state.pool, world).await?;
        }
        KeyTarget::bound(&state, bound).await?
    };
    match target.decide(&state, &sf.signer).await {
        PermissionDecision::Allowed => {}
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
            return Err(forbidden(NOT_AUTHORIZED_TO_STREAM_MSG));
        }
        PermissionDecision::Refused(why) => return Err(why.into()),
    }
    let (place_id, world_room, room) = (
        target.place_id.as_str(),
        target.world_room.as_deref(),
        target.room.as_str(),
    );
    let ingress = state.ingress();
    // A key the place no longer holds (its scene redeployed, its minter
    // without the place) is never the one a verb answers or acts on.
    let revoked = recheck_target(&state, place_id, world_room).await;
    match method {
        Method::GET => match active_access(&state.pool, place_id, world_room).await? {
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
            (place_id, world_room),
            room,
            &sf.signer,
            &target.admin_place_key,
        )
        .await
        .map(|resp| Json(resp).into_response()),
        Method::PUT => reset_access(
            &state,
            &ingress,
            (place_id, world_room),
            room,
            &sf.signer,
            &target.admin_place_key,
        )
        .await
        .map(|resp| Json(resp).into_response()),
        Method::DELETE => remove_access(&state, place_id, world_room, &revoked).await,
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

/// The credential a stream-key verb acts on and the authority it decides on.
/// Each scene has its own key, streaming into its own room: a Genesis scene's
/// is stored under the catalogue place based on its base parcel, a world
/// scene's under its place (`ActiveDeployment::place_key`, where `/scene-admin`
/// grants and scene bans live too, kept across a redeploy of that place).
pub(super) struct KeyTarget<'a> {
    pub place_id: String,
    /// The room the scene's key streams into (empty for a Genesis revocation).
    pub room: String,
    /// A world scene's room: its key is the one streaming there, also one
    /// minted under the world's catalogue place before world keys were per
    /// scene, until a reset or renewal replaces it or it expires
    /// ([`active_access`]).
    pub world_room: Option<String>,
    /// The bound scene's admin place, saved on a minted credential so its
    /// notices reach that scene's admins after its identity retires.
    pub admin_place_key: String,
    authority: PlaceLookup<'a>,
    /// A world scene: the world-wide lookup and the scene's admin key
    /// ([`world_scene_admin`]).
    world: Option<(PlaceLookup<'a>, String)>,
}

impl<'a> KeyTarget<'a> {
    /// The scene a request is bound to ([`Context::resolve`]). 404 for a
    /// Genesis scene the catalogue lists no place for.
    pub async fn bound(state: &'a CommsState, bound: Context<'a>) -> Result<Self, ApiError> {
        if let Some((world, _)) = &bound.world_scene {
            let key = bound.place.admin_key().to_owned();
            return Ok(Self::world(state, world, key, bound.footprint, bound.room));
        }
        let base = bound.footprint.base_position.clone().unwrap_or_default();
        let place_id = resolve_place_id(&state.pool, &base, true)
            .await?
            .ok_or_else(|| ApiError::not_found("place not found for this scene"))?;
        Ok(Self {
            place_id,
            room: bound.room,
            world_room: None,
            admin_place_key: bound.place.admin_key().to_owned(),
            authority: bound.place,
            world: None,
        })
    }

    /// The world scene `deployment`, whose room is `room`.
    pub fn deployed(
        state: &'a CommsState,
        world: &WorldName,
        deployment: &crate::rooms::ActiveDeployment,
        room: String,
    ) -> Self {
        let footprint = scene::PlaceInfo {
            world: true,
            world_name: Some(world.key().to_owned()),
            positions: deployment.parcels.iter().map(Parcel::canonical).collect(),
            base_position: Some(deployment.base.canonical()),
        };
        Self::world(state, world, deployment.place_key(), footprint, room)
    }

    /// The world-wide lookup of `world` under `place_id`: the world with no
    /// footprint, which its owner and the grantees with no parcel restriction
    /// hold, and no `/scene-admin` grant. A world scene admin's granter is
    /// decided on it, for the key and for the key's notices.
    pub fn world_wide(state: &'a CommsState, place_id: &str, world: &WorldName) -> PlaceLookup<'a> {
        let footprint = scene::PlaceInfo {
            world: true,
            world_name: Some(world.key().to_owned()),
            positions: Vec::new(),
            base_position: None,
        };
        PlaceLookup::resolved(state, place_id, footprint, scene::NO_ADMINS)
    }

    /// A world scene's key decides on the scene's footprint without explicit
    /// admins: the world's owner and a deployment or streaming grantee whose
    /// grant has no parcel restriction or overlaps the scene's parcels. The
    /// scene's explicit admins are decided apart ([`world_scene_admin`]).
    fn world(
        state: &'a CommsState,
        world: &WorldName,
        key: String,
        footprint: scene::PlaceInfo,
        room: String,
    ) -> Self {
        Self {
            authority: PlaceLookup::resolved(state, &key, footprint, scene::NO_ADMINS),
            world: Some((Self::world_wide(state, &key, world), key.clone())),
            world_room: Some(room.clone()),
            admin_place_key: key.clone(),
            place_id: key,
            room,
        }
    }

    pub async fn decide(&self, state: &CommsState, signer: &Address) -> PermissionDecision {
        let decision = scene::scene_manager_decision(state, &self.authority, signer).await;
        match &self.world {
            Some((world_wide, scene)) if lacks_authority(&decision) => {
                world_scene_admin(state, world_wide, scene, signer, decision).await
            }
            _ => decision,
        }
    }
}

pub(super) fn lacks_authority(decision: &PermissionDecision) -> bool {
    matches!(
        decision,
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. })
    )
}

/// After the scene's decision `refused` the signer for lack of authority: an
/// explicit admin of the world `scene` holds that scene's key when whoever
/// granted it holds world-wide authority (`world_wide`: the owner, a grantee
/// without a parcel restriction). A grantee restricted to some parcels may
/// grant `/scene-admin` on its scene, so an admin it granted (or one granted
/// by another scene admin) stays refused: the grant never widens its
/// granter's authority.
async fn world_scene_admin(
    state: &CommsState,
    world_wide: &PlaceLookup<'_>,
    scene: &str,
    signer: &Address,
    refused: PermissionDecision,
) -> PermissionDecision {
    let granter = match state.moderation.active_admin_granter(scene, signer).await {
        Ok(Some(granter)) => granter,
        Ok(None) => return refused,
        Err(error) => return PermissionDecision::undetermined("social.scene_admins", &error),
    };
    match scene::scene_manager_decision(state, world_wide, &granter).await {
        PermissionDecision::Allowed => PermissionDecision::Allowed,
        decision if lacks_authority(&decision) => refused,
        undetermined => undetermined,
    }
}

/// The key a revocation acts on. Revocation needs no current deployment: in
/// a world, the signed scene when it is current, else the scene standing on
/// the signed parcel now (its place keeps its key across a redeploy); a
/// Genesis place by the signed parcel, decided by [`revocation_authority`].
async fn revocation_target<'a>(
    state: &'a CommsState,
    metadata: &serde_json::Value,
) -> Result<KeyTarget<'a>, ApiError> {
    let realm = Context::signed_realm(metadata, &state.settings.realm_name)?;
    let parcel = meta_str(metadata, "parcel");
    let not_found = || ApiError::not_found("place not found for this scene");
    let Some(world) = realm.world() else {
        let parcel = parcel
            .ok_or_else(|| ApiError::bad_request("invalid signed-fetch request, no parcel"))?;
        let place_id = resolve_place_id(&state.pool, &parcel, false)
            .await?
            .ok_or_else(not_found)?;
        let authority =
            revocation_authority(state, metadata, &realm, Some(&parcel), &place_id).await?;
        return Ok(KeyTarget {
            place_id,
            room: String::new(),
            world_room: None,
            admin_place_key: String::new(),
            authority,
            world: None,
        });
    };
    match Context::resolve(state, metadata).await {
        Ok(bound) => return KeyTarget::bound(state, bound).await,
        Err(error) if error.code >= 500 => return Err(error),
        Err(_) => {}
    }
    let parcel = parcel
        .and_then(|raw| Parcel::parse(&raw).ok())
        .ok_or_else(not_found)?;
    let request = RoomRequest::Parcel {
        realm: &realm,
        parcel,
    };
    match state.scenes.resolve(request).await {
        Ok(current) => match current.deployment.as_ref() {
            Some(deployment) => Ok(KeyTarget::deployed(state, world, deployment, current.room)),
            None => Err(not_found()),
        },
        Err(crate::rooms::ResolveError::Db(error)) => Err(scene_identity_unavailable(&error)),
        Err(_) => Err(not_found()),
    }
}

/// The lookup a Genesis revocation is authorized on: the signed scene when it
/// is the current deployment, as for the other verbs; else the scene standing
/// on the signed parcel now, so the admins of the scene that replaced it can
/// still revoke the key; with no scene there, the place's owner alone. A
/// resolver or catalogue that cannot be read is an error (503), never one of
/// the narrower lookups.
async fn revocation_authority<'a>(
    state: &'a CommsState,
    metadata: &serde_json::Value,
    realm: &RealmId,
    parcel: Option<&str>,
    place_id: &str,
) -> Result<PlaceLookup<'a>, ApiError> {
    let stored = PlaceLookup::new(state, place_id, scene::NO_ADMINS);
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
        Err(crate::rooms::ResolveError::Db(error)) => {
            return Err(scene_identity_unavailable(&error));
        }
        // No usable scene stands on the parcel: the place's owner alone.
        Err(_) => return Ok(stored),
    };
    Ok(scene_authority(state, &current, place_id)
        .await?
        .unwrap_or(stored))
}

/// Rechecks the keys a verb may answer ([`cast::recheck_keys`]): the place's,
/// and, for a world scene, the one streaming into its room.
pub(super) async fn recheck_target(
    state: &SharedState,
    place_id: &str,
    world_room: Option<&str>,
) -> Vec<cast::RevokedKey> {
    let mut revoked = cast::recheck_keys(state, cast::Keys::Place(place_id)).await;
    if let Some(room) = world_room {
        revoked.extend(cast::recheck_keys(state, cast::Keys::Room(room)).await);
    }
    revoked
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
    (place_id, world_room): (&str, Option<&str>),
    room: &str,
    signer: &Address,
    admin_place_key: &str,
) -> Result<SceneStreamAccessResponse, ApiError> {
    // The caller has rechecked the place's keys ([`recheck_target`]).
    free_scene_place(state, (place_id, world_room), room).await?;
    if let Some(existing) = active_access(&state.pool, place_id, world_room).await? {
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

/// The Genesis place holding `parcel`, preferring the one based there. With
/// `based_there` only a place based on `parcel` counts, so a catalogue that
/// has not caught up with a redeploy never lends another scene's admins.
/// Whether `world` has a catalogue place (`explore.place` or
/// `explore.place_world_local`). A blocked or removed world has none.
pub(super) async fn world_catalogued(pool: &PgPool, world: &WorldName) -> Result<bool, ApiError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM explore.place WHERE world AND lower(world_name) = lower($1)) \
         OR EXISTS (SELECT 1 FROM explore.place_world_local WHERE world AND lower(world_name) = lower($1))",
    )
    .bind(world.display())
    .fetch_one(pool)
    .await?)
}

/// 404 unless `world` has a catalogue place: a world scene mints, resets and
/// presents only while its world is listed. Revocation is never gated.
pub(super) async fn require_world_place(pool: &PgPool, world: &WorldName) -> Result<(), ApiError> {
    match world_catalogued(pool, world).await? {
        true => Ok(()),
        false => Err(ApiError::not_found("place not found for this scene")),
    }
}

async fn resolve_place_id(
    pool: &PgPool,
    parcel: &str,
    based_there: bool,
) -> Result<Option<String>, ApiError> {
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

/// The active credential a verb acts on. A Genesis key is its place's. A
/// world scene's (`world_room`, its room) is the one streaming into that
/// room ([`ROOM_KEY_ORDER`]): a row under the scene's place that
/// streams into another room is another scene's (a world key stored under a
/// catalogue id that an imported identity equals), never this scene's.
async fn active_access(
    pool: &PgPool,
    place_id: &str,
    world_room: Option<&str>,
) -> Result<Option<AccessRow>, sqlx::Error> {
    match world_room {
        None => sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {ACCESS_COLUMNS} FROM social.scene_stream_access WHERE place_id = $1 AND active"
        )))
        .bind(place_id)
        .fetch_optional(pool)
        .await,
        Some(room) => sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {ACCESS_COLUMNS} FROM social.scene_stream_access WHERE room_id = $1 AND active \
                 ORDER BY {ROOM_KEY_ORDER} LIMIT 1"
        )))
        .bind(room)
        .fetch_optional(pool)
        .await,
    }
}

/// Before a world scene stores a key under its place: a key already there
/// that streams into another room is another scene's (a world key stored
/// under a catalogue id that this scene's imported identity equals). It
/// moves to the place of its room's scene, which keeps its stream; when it
/// cannot (its room has no scene, or that scene has a key of its own), it is
/// revoked while idle ([`revoke_held`]). Before any ingress is minted, a
/// live one answers 409 as a live broadcast does, and a change meanwhile
/// 409 to retry. A Genesis key's place holds no other scene's key.
async fn free_scene_place(
    state: &SharedState,
    (place_id, world_room): (&str, Option<&str>),
    room: &str,
) -> Result<(), ApiError> {
    if world_room.is_none() {
        return Ok(());
    }
    let held: Option<(Uuid, Option<String>)> = sqlx::query_as(
        "SELECT id, room_id FROM social.scene_stream_access \
         WHERE active AND place_id = $1 AND room_id IS DISTINCT FROM $2",
    )
    .bind(place_id)
    .bind(room)
    .fetch_optional(&state.pool)
    .await?;
    let Some((id, other)) = held else {
        return Ok(());
    };
    let owner = match other.as_deref() {
        Some(other) => match state
            .scenes
            .resolve(RoomRequest::NamedWorldScene { room: other })
            .await
        {
            // A scene retired within the eviction window still resolves, with
            // no identity: its entity id is no place a verb reaches, so the
            // row is revoked instead.
            Ok(resolved) => resolved
                .deployment
                .filter(|deployment| deployment.namespace.is_some())
                .map(|deployment| deployment.place_key()),
            Err(crate::rooms::ResolveError::Db(error)) => {
                return Err(scene_identity_unavailable(&error));
            }
            Err(_) => None,
        },
        None => None,
    };
    if let Some(owner) = owner.filter(|owner| owner != place_id) {
        // The move bumps the version, so a revocation prepared before it fails.
        let moved = sqlx::query(
            "UPDATE social.scene_stream_access \
             SET place_id = $3, admin_place_key = COALESCE(admin_place_key, $3), \
                 streaming_state_version = streaming_state_version + 1 \
             WHERE id = $1 AND active AND place_id = $2 \
             AND NOT EXISTS (SELECT 1 FROM social.scene_stream_access WHERE active AND place_id = $3)",
        )
        .bind(id)
        .bind(place_id)
        .bind(&owner)
        .execute(&state.pool)
        .await;
        match moved {
            Ok(done) if done.rows_affected() == 1 => return Ok(()),
            Ok(_) => {}
            // A concurrent mint took the other place.
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {}
            Err(error) => return Err(error.into()),
        }
    }
    match revoke_held(state, id, place_id).await? {
        Held::Freed => Ok(()),
        Held::Live => Err(renewal_conflict()),
        Held::Changed => Err(concurrent_change()),
    }
}

/// What [`revoke_held`] did with a key held under a scene's place.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Held {
    /// Revoked, or no longer under the place (moved meanwhile, or gone).
    Freed,
    /// Streaming: it stays.
    Live,
    /// It changed between the read and the revocation.
    Changed,
}

/// Revokes key `id` while it is still active under `place_id` and idle. A key
/// a concurrent request moved to its own scene's place is that scene's, and
/// stays. Idle means LiveKit, not only the flag, says its ingress is not
/// publishing: a lost `ingress_started` leaves the flag down on a live
/// broadcast, which is raised and kept. An ingress state that cannot be read
/// keeps the key.
pub(crate) async fn revoke_held(
    state: &SharedState,
    id: Uuid,
    place_id: &str,
) -> Result<Held, ApiError> {
    let current: Option<(bool, i64, Option<String>)> = sqlx::query_as(
        "SELECT streaming, streaming_state_version, ingress_id FROM social.scene_stream_access \
         WHERE id = $1 AND active AND place_id = $2",
    )
    .bind(id)
    .bind(place_id)
    .fetch_optional(&state.pool)
    .await?;
    let (version, ingress_id) = match current {
        None => return Ok(Held::Freed),
        Some((true, _, _)) => return Ok(Held::Live),
        Some((false, version, ingress_id)) => (version, ingress_id.unwrap_or_default()),
    };
    if !ingress_id.is_empty() {
        match state.ingress().is_ingress_streaming(&ingress_id).await {
            Ok(Some(false)) => {}
            Ok(Some(true)) => {
                jobs::raise(&state.pool, id, version).await?;
                return Ok(Held::Live);
            }
            Ok(None) | Err(_) => {
                tracing::warn!(place_id, access_id = %id, "ingress state unavailable; the held key stays");
                return Ok(Held::Live);
            }
        }
    }
    revoke_at(state, id, place_id, version).await
}

/// [`revoke_held`]'s revocation, conditional on the version it read.
pub(crate) async fn revoke_at(
    state: &SharedState,
    id: Uuid,
    place_id: &str,
    version: i64,
) -> Result<Held, ApiError> {
    let Some(revocation) = jobs::prepare(state, id, jobs::CleanupCommand::Revoke).await? else {
        return Ok(Held::Freed);
    };
    if revocation.version() != version || !revocation.record(&state.pool).await? {
        return Ok(Held::Changed);
    }
    tracing::info!(place_id, access_id = %id, "revoked a key held under this scene's place for another room");
    settle_cleanup(state, id, place_id);
    Ok(Held::Freed)
}

/// A scene resolver whose store cannot be read: 503, as every authority
/// source that cannot be read.
pub(super) fn scene_identity_unavailable(error: &sqlx::Error) -> ApiError {
    AuthorityNotEstablished::store_unavailable("worlds.world_scenes", error).into()
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
        if !prepared.record(&mut *tx).await? {
            return Ok(None);
        }
        let Some(row) =
            insert_access(&mut *tx, place_id, room, signer, admin_place_key, &info).await?
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

/// The active credentials a removed place ends: a removed world's are every
/// key streaming into one of its scene rooms (each scene's, and a key stored
/// under the world's catalogue place); a Genesis place's is the one stored
/// under it, never a world scene's key with the same id. `world` is the
/// world's key.
const REMOVED_PLACE_KEYS: &str = "active AND CASE WHEN $2::text IS NULL \
     THEN place_id = $1 AND (room_id IS NULL OR NOT starts_with(room_id, $3)) \
     ELSE place_id = $1 OR starts_with(room_id, $2) END";

/// Whether a removed place has a key with a live ingress ([`REMOVED_PLACE_KEYS`]).
pub(crate) async fn removed_place_has_ingress<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
    place_id: &str,
    world: Option<&str>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS(SELECT 1 FROM social.scene_stream_access \
         WHERE {REMOVED_PLACE_KEYS} AND ingress_id <> '')"
    )))
    .bind(place_id)
    .bind(world.map(crate::rooms::world_scene_room_prefix))
    .bind(crate::rooms::SCENE_ROOM_PREFIX)
    .fetch_one(executor)
    .await
}

/// Ends a removed place's credentials ([`REMOVED_PLACE_KEYS`]) in the
/// removal's transaction, as a revocation does (its cleanup deletes the
/// ingress and removes the Cast publishers) but with no key notice: the
/// removal tells the owner. Returns the credentials to settle after the commit.
pub(crate) async fn end_removed_place(
    tx: &mut sqlx::PgConnection,
    place_id: &str,
    world: Option<&str>,
    removed_at: NaiveDateTime,
) -> Result<Vec<Uuid>, sqlx::Error> {
    // A key made after the removal belongs to the place that came back.
    let rows: Vec<(Uuid, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT id, streaming_state_version FROM social.scene_stream_access \
         WHERE {REMOVED_PLACE_KEYS} AND created_at <= $4 ORDER BY id FOR UPDATE"
    )))
    .bind(place_id)
    .bind(world.map(crate::rooms::world_scene_room_prefix))
    .bind(crate::rooms::SCENE_ROOM_PREFIX)
    .bind(removed_at)
    .fetch_all(&mut *tx)
    .await?;
    let command = jobs::CleanupCommand::PlaceRemoval;
    for (id, version) in &rows {
        jobs::enqueue_prepared(&mut *tx, *id, command, *version, None, None, None).await?;
    }
    Ok(rows.into_iter().map(|(id, _)| id).collect())
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

/// Inserts the place's active credential for `info`'s ingress; `None` when
/// the place already has one.
async fn insert_access<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
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
    // A failed insertion discards the ingress minted for it.
    let inserted = match insert_access(pool, place_id, room, signer, admin_place_key, &info).await {
        Ok(inserted) => inserted,
        Err(error) => {
            discard_ingress(ingress, &info, place_id).await;
            return Err(error);
        }
    };
    if let Some(row) = inserted {
        return Ok(row.response());
    }
    let winner = active_access(pool, place_id, None).await?.ok_or_else(|| {
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
    (place_id, world_room): (&str, Option<&str>),
    room: &str,
    signer: &Address,
    admin_place_key: &str,
) -> Result<SceneStreamAccessResponse, ApiError> {
    let result = async {
        free_scene_place(state, (place_id, world_room), room).await?;
        let row = active_access(&state.pool, place_id, world_room)
            .await?
            .ok_or_else(|| ApiError::not_found(STREAM_ACCESS_NOT_FOUND_MSG))?;
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
        // Preserve not-found, retry (409) and unreadable-source (503)
        // responses; other swap failures use the reset error.
        if matches!(error.code, 404 | 409 | 503) {
            return error;
        }
        tracing::error!(place_id, error = %error, "scene stream access reset failed");
        ApiError::http(500, STREAM_RESET_FAILED_MSG)
    })
}

/// Whether world scene `room` names a deployment, current or displaced, of
/// the scene place `place_id` (`explore.world_scene_place`,
/// `explore.world_scene_binding`).
async fn room_of_place(pool: &PgPool, room: &str, place_id: &str) -> Result<bool, sqlx::Error> {
    let Some((world, scene)) = crate::rooms::decode_world_scene_room(room) else {
        return Ok(false);
    };
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM explore.world_scene_place \
             WHERE world_name = $1 AND entity_id = $2 AND identity::text = $3) \
         OR EXISTS (SELECT 1 FROM explore.world_scene_binding \
             WHERE world_name = $1 AND entity_id = $2 AND identity::text = $3)",
    )
    .bind(world.key())
    .bind(scene.as_str())
    .bind(place_id)
    .fetch_one(pool)
    .await
}

async fn remove_access(
    state: &SharedState,
    place_id: &str,
    world_room: Option<&str>,
    revoked: &[cast::RevokedKey],
) -> Result<Response, ApiError> {
    // A key of this scene that the request's recheck ended (its scene
    // replaced by a redeploy, its minter without the place) was revoked by
    // this request: 204 as for any other. A world key streaming into another
    // scene's room is not this scene's; a replaced one is when its room was a
    // deployment of this scene's place.
    let mut ours = false;
    for key in revoked {
        ours = match (world_room, key.room.as_deref()) {
            (None, _) => true,
            (Some(room), Some(revoked)) if revoked == room => true,
            (Some(_), Some(revoked)) if key.replaced => {
                room_of_place(&state.pool, revoked, place_id).await?
            }
            _ => false,
        };
        if ours {
            break;
        }
    }
    let Some(row) = active_access(&state.pool, place_id, world_room).await? else {
        return match ours {
            false => Err(ApiError::not_found(
                "no active stream access for this scene",
            )),
            true => Ok(StatusCode::NO_CONTENT.into_response()),
        };
    };
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

#[cfg(test)]
mod scene_key_tests {
    use super::*;
    use crate::world_identity::SceneId;

    async fn key(pool: &PgPool, place: &str, room: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO social.scene_stream_access \
                 (place_id, streaming_url, streaming_key, ingress_id, room_id, generated_by, active) \
             VALUES ($1, 'rtmp://example.org', $2, '', $3, 'test', true) RETURNING id",
        )
        .bind(place)
        .bind(Uuid::new_v4().to_string())
        .bind(room)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn room(world: &WorldName, scene: &str) -> String {
        crate::rooms::worlds_service_scene_room(world, &SceneId::parse(scene).unwrap())
    }

    /// Two active keys streaming into one room (a mixed rollout): the verbs
    /// and the presenter routes both take the one under the scene's place.
    #[tokio::test]
    async fn the_verbs_and_the_presenter_take_the_same_key_of_a_room() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let tag = Uuid::new_v4().simple().to_string();
        let world = WorldName::parse(&format!("order{}.dcl.eth", &tag[..10])).unwrap();
        let scene_room = room(&world, "SceneA");
        let (scene, catalogue) = (format!("s-{tag}"), format!("c-{tag}"));
        let own = key(&pool, &scene, &scene_room).await;
        let newer = key(&pool, &catalogue, &scene_room).await;
        sqlx::query(
            "UPDATE social.scene_stream_access SET admin_place_key = $2 WHERE id = ANY($1)",
        )
        .bind(vec![own, newer])
        .bind(&scene)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE social.scene_stream_access SET created_at = created_at + interval '1 minute' WHERE id = $1")
            .bind(newer)
            .execute(&pool)
            .await
            .unwrap();
        let verbs = active_access(&pool, &scene, Some(&scene_room))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(verbs.id, own);
        let identity = |access: Uuid| format!("stream:x:{access}:{}", Uuid::new_v4());
        assert!(
            cast::publisher_credential_current(&pool, &scene_room, &identity(own))
                .await
                .unwrap()
        );
        assert!(
            !cast::publisher_credential_current(&pool, &scene_room, &identity(newer))
                .await
                .unwrap()
        );
    }

    /// A revocation whose recheck ended a replaced key answers 204 only when
    /// that key's room was a deployment of the scene's place; a key of
    /// another scene's room under a colliding place id answers 404.
    #[tokio::test]
    async fn a_revocation_owns_a_replaced_key_only_of_its_own_scene() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let state = scratch_state(pool.clone());
        let tag = Uuid::new_v4().simple().to_string();
        let world = WorldName::parse(&format!("own{}.dcl.eth", &tag[..10])).unwrap();
        let identity = Uuid::new_v4();
        sqlx::query("INSERT INTO explore.world_scene_place (world_name, identity, entity_id, parcels) VALUES ($1, $2, 'OldA', '{\"0,0\"}')")
            .bind(world.key()).bind(identity).execute(&pool).await.unwrap();
        // A redeploy of the place retains the old binding.
        sqlx::query("UPDATE explore.world_scene_place SET entity_id = 'NewA' WHERE identity = $1")
            .bind(identity)
            .execute(&pool)
            .await
            .unwrap();
        let place = identity.to_string();
        let current = room(&world, "NewA");
        let revoked = |scene: &str| {
            vec![cast::RevokedKey {
                room: Some(room(&world, scene)),
                replaced: true,
            }]
        };
        let error = remove_access(&state, &place, Some(&current), &revoked("SceneB"))
            .await
            .unwrap_err();
        assert_eq!(error.code, 404, "another scene's key");
        let ok = remove_access(&state, &place, Some(&current), &revoked("OldA"))
            .await
            .unwrap();
        assert_eq!(ok.status(), StatusCode::NO_CONTENT, "its replaced key");
    }

    /// An older binary's world key, under the catalogue place with its scene
    /// saved, is refused on insert; per-scene, imported and Genesis rows are not.
    #[tokio::test]
    async fn a_world_key_under_another_place_than_its_scene_is_refused() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let tag = Uuid::new_v4().simple().to_string();
        let world = WorldName::parse(&format!("fence{}.dcl.eth", &tag[..10])).unwrap();
        let insert = |place: String, room: String, admin: Option<String>| {
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "INSERT INTO social.scene_stream_access \
                         (place_id, streaming_url, streaming_key, ingress_id, room_id, generated_by, admin_place_key) \
                     VALUES ($1, 'rtmp://example.org', $2, '', $3, 'test', $4)",
                )
                .bind(place)
                .bind(Uuid::new_v4().to_string())
                .bind(room)
                .bind(admin)
                .execute(&pool)
                .await
            }
        };
        let refused = insert(
            format!("c-{tag}"),
            room(&world, "SceneA"),
            Some(format!("s-{tag}")),
        )
        .await
        .unwrap_err();
        let code = refused.as_database_error().and_then(|e| e.code());
        assert_eq!(code.as_deref(), Some("23514"));
        insert(
            format!("s-{tag}"),
            room(&world, "SceneA"),
            Some(format!("s-{tag}")),
        )
        .await
        .unwrap();
        insert(format!("i-{tag}"), room(&world, "SceneB"), None)
            .await
            .unwrap();
        insert(
            format!("g-{tag}"),
            format!("scene:main:{tag}"),
            Some(format!("0,0-{tag}")),
        )
        .await
        .unwrap();
    }

    /// A world scene reads the key streaming into its room, its own first,
    /// else one under another place (the world's, from before keys were per
    /// scene); a key under its place streaming elsewhere is not its own.
    #[tokio::test]
    async fn a_world_scene_reads_its_key_then_the_one_streaming_into_its_room() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let tag = Uuid::new_v4().simple().to_string();
        let world = WorldName::parse(&format!("keys{}.dcl.eth", &tag[..10])).unwrap();
        let (room_a, room_b) = (room(&world, "SceneA"), room(&world, "SceneB"));
        let (place_a, place_b) = (format!("a-{tag}"), format!("b-{tag}"));
        let legacy = key(&pool, &format!("world-{tag}"), &room_a).await;
        let read = |place: String, room: Option<String>| {
            let pool = pool.clone();
            async move {
                active_access(&pool, &place, room.as_deref())
                    .await
                    .unwrap()
                    .map(|row| row.id)
            }
        };
        assert_eq!(
            read(place_a.clone(), Some(room_a.clone())).await,
            Some(legacy)
        );
        assert_eq!(read(place_b.clone(), Some(room_b.clone())).await, None);
        assert_eq!(
            read(place_a.clone(), None).await,
            None,
            "a Genesis read is by place"
        );
        let own = key(&pool, &place_a, &room_a).await;
        assert_eq!(
            read(place_a, Some(room_a)).await,
            Some(own),
            "its own key first"
        );
        // A key stored under scene C's place (a catalogue id C's imported
        // identity equals) that streams into B's room is B's, never C's.
        let place_c = format!("c-{tag}");
        let collided = key(&pool, &place_c, &room_b).await;
        assert_eq!(read(place_c, Some(room(&world, "SceneC"))).await, None);
        assert_eq!(read(place_b, Some(room_b)).await, Some(collided));
    }

    fn scratch_state(pool: PgPool) -> SharedState {
        use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
        std::sync::Arc::new(CommsState::new(
            pool,
            crate::http::Outbound::for_tests(),
            crate::gatekeeper::Settings::for_tests(),
            None,
            None,
            std::sync::Arc::new(StubPolicy::default()),
            std::sync::Arc::new(StubScenes::default()),
            std::sync::Arc::new(crate::presence::Presence::new()),
            std::sync::Arc::new(StubDirectory::default()),
        ))
    }

    /// A key held under another scene's place is revoked only while it is
    /// still there, idle and unchanged: one a concurrent request moved to its
    /// own scene's place stays that scene's, a live one stays, and a change
    /// after the read stops the revocation.
    #[tokio::test]
    async fn a_held_key_is_revoked_only_while_idle_and_still_held() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let state = scratch_state(pool.clone());
        let tag = Uuid::new_v4().simple().to_string();
        let world = WorldName::parse(&format!("held{}.dcl.eth", &tag[..10])).unwrap();
        let (place_a, place_b) = (format!("a-{tag}"), format!("b-{tag}"));
        let room_b = room(&world, "SceneB");
        let active = |id: Uuid| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, bool>(
                    "SELECT active FROM social.scene_stream_access WHERE id = $1",
                )
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        let set = |sql: &'static str, id: Uuid| {
            let pool = pool.clone();
            async move {
                sqlx::query(sql).bind(id).execute(&pool).await.unwrap();
            }
        };
        // A concurrent request moved it first; the stale revocation leaves it.
        let moved = key(&pool, &place_a, &room_b).await;
        sqlx::query("UPDATE social.scene_stream_access SET place_id = $2, streaming_state_version = streaming_state_version + 1 WHERE id = $1")
            .bind(moved)
            .bind(&place_b)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            revoke_held(&state, moved, &place_a).await.unwrap(),
            Held::Freed
        );
        assert!(active(moved).await, "the moved key is the other scene's");
        set(
            "UPDATE social.scene_stream_access SET active = false WHERE id = $1",
            moved,
        )
        .await;
        // Live: it stays.
        let live = key(&pool, &place_a, &room_b).await;
        set(
            "UPDATE social.scene_stream_access SET streaming = true WHERE id = $1",
            live,
        )
        .await;
        assert_eq!(
            revoke_held(&state, live, &place_a).await.unwrap(),
            Held::Live
        );
        assert!(active(live).await);
        set(
            "UPDATE social.scene_stream_access SET active = false WHERE id = $1",
            live,
        )
        .await;
        // Changed after the read: the revocation is not stored.
        let changed = key(&pool, &place_a, &room_b).await;
        let read: i64 = sqlx::query_scalar(
            "SELECT streaming_state_version FROM social.scene_stream_access WHERE id = $1",
        )
        .bind(changed)
        .fetch_one(&pool)
        .await
        .unwrap();
        set("UPDATE social.scene_stream_access SET streaming_state_version = streaming_state_version + 1 WHERE id = $1", changed).await;
        assert_eq!(
            revoke_at(&state, changed, &place_a, read).await.unwrap(),
            Held::Changed
        );
        assert!(active(changed).await);
        // Idle and still held: revoked.
        assert_eq!(
            revoke_held(&state, changed, &place_a).await.unwrap(),
            Held::Freed
        );
        assert!(!active(changed).await);
    }

    /// The ingress check of a removal reads the keys the removal ends.
    #[tokio::test]
    async fn a_removal_sees_the_live_ingress_of_the_keys_it_ends() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let tag = Uuid::new_v4().simple().to_string();
        let world = WorldName::parse(&format!("live{}.dcl.eth", &tag[..10])).unwrap();
        let catalogue = format!("world-{tag}");
        let idle = key(&pool, &format!("a-{tag}"), &room(&world, "SceneA")).await;
        let has = |place: String, world: Option<String>| {
            let pool = pool.clone();
            async move {
                removed_place_has_ingress(&pool, &place, world.as_deref())
                    .await
                    .unwrap()
            }
        };
        let world_key = Some(world.key().to_owned());
        assert!(
            !has(catalogue.clone(), world_key.clone()).await,
            "no ingress yet"
        );
        sqlx::query("UPDATE social.scene_stream_access SET ingress_id = 'IN_live' WHERE id = $1")
            .bind(idle)
            .execute(&pool)
            .await
            .unwrap();
        assert!(has(catalogue, world_key).await, "a scene key of the world");
        assert!(
            !has(format!("a-{tag}"), None).await,
            "a Genesis place with the same id is another place"
        );
    }

    /// A reset's replacement is under the same place, or, in a world scene
    /// room, any key streaming into that room; a Genesis room's key under
    /// another place replaces nothing.
    #[tokio::test]
    async fn a_replacement_is_the_places_or_the_world_scene_rooms() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let tag = Uuid::new_v4().simple().to_string();
        let world = WorldName::parse(&format!("reset{}.dcl.eth", &tag[..10])).unwrap();
        let world_room = room(&world, "SceneA");
        let genesis_room = format!("scene:main:{tag}");
        let retire = |id: Uuid| {
            let pool = pool.clone();
            async move {
                sqlx::query("UPDATE social.scene_stream_access SET active = false WHERE id = $1")
                    .bind(id)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        };
        let legacy = key(&pool, &format!("world-{tag}"), &world_room).await;
        retire(legacy).await;
        assert!(
            !jobs::replaced(&pool, &format!("world-{tag}"), legacy)
                .await
                .unwrap()
        );
        key(&pool, &format!("a-{tag}"), &world_room).await;
        assert!(
            jobs::replaced(&pool, &format!("world-{tag}"), legacy)
                .await
                .unwrap()
        );
        // Another scene's key under a colliding place id is no replacement.
        let collided = key(&pool, &format!("p-{tag}"), &room(&world, "SceneB")).await;
        retire(collided).await;
        key(&pool, &format!("p-{tag}"), &room(&world, "SceneC")).await;
        assert!(
            !jobs::replaced(&pool, &format!("p-{tag}"), collided)
                .await
                .unwrap()
        );
        let old = key(&pool, &format!("g-{tag}"), &genesis_room).await;
        retire(old).await;
        key(&pool, &format!("h-{tag}"), &genesis_room).await;
        assert!(
            !jobs::replaced(&pool, &format!("g-{tag}"), old)
                .await
                .unwrap()
        );
        key(
            &pool,
            &format!("g-{tag}"),
            &format!("scene:main:other{tag}"),
        )
        .await;
        assert!(
            jobs::replaced(&pool, &format!("g-{tag}"), old)
                .await
                .unwrap()
        );
    }

    /// A removed world ends every key streaming into one of its scene rooms,
    /// its catalogue place's included; a removed Genesis place ends its own
    /// key alone, never a world scene's key stored under the same id.
    #[tokio::test]
    async fn a_removed_world_ends_each_scene_key_and_a_genesis_place_its_own() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let tag = Uuid::new_v4().simple().to_string();
        let world = WorldName::parse(&format!("gone{}.dcl.eth", &tag[..10])).unwrap();
        let other = WorldName::parse(&format!("gone{}x.dcl.eth", &tag[..10])).unwrap();
        let catalogue = format!("world-{tag}");
        let scene_a = key(&pool, &format!("a-{tag}"), &room(&world, "SceneA")).await;
        let scene_b = key(&pool, &format!("b-{tag}"), &room(&world, "SceneB")).await;
        let legacy = key(&pool, &catalogue, &room(&world, "SceneC")).await;
        let elsewhere = key(&pool, &format!("c-{tag}"), &room(&other, "SceneA")).await;
        let genesis_id = format!("g-{tag}");
        let world_scene = key(&pool, &genesis_id, &room(&world, "SceneD")).await;
        let genesis = key(&pool, &format!("h-{tag}"), &format!("scene:main:{tag}")).await;
        let later = chrono::Utc::now().naive_utc() + chrono::Duration::minutes(1);
        let ended = |place: String, world: Option<String>| {
            let pool = pool.clone();
            async move {
                let mut tx = pool.begin().await.unwrap();
                let ids = end_removed_place(&mut tx, &place, world.as_deref(), later)
                    .await
                    .unwrap();
                tx.rollback().await.unwrap();
                ids
            }
        };
        let mut expected = vec![scene_a, scene_b, legacy, world_scene];
        expected.sort();
        assert_eq!(
            ended(catalogue, Some(world.key().to_owned())).await,
            expected
        );
        assert_eq!(ended(genesis_id, None).await, Vec::<Uuid>::new());
        assert_eq!(ended(format!("h-{tag}"), None).await, vec![genesis]);
        assert!(!expected.contains(&elsewhere));
    }
}
