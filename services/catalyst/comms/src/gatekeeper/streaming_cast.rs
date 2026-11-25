//! Cast 2.0 (`/cast/*`), upstream comms-gatekeeper's `handlers/cast` and
//! `logic/cast` at b42d45c7b: a scene admin turns the place's stream
//! credential (the `social.scene_stream_access` row `/scene-stream-access`
//! keeps) into a stream link, and the link's streaming key then admits a
//! WebRTC streamer and a presentation bot to the scene's own room, where
//! signed-in viewers watch. Presenters are a `presenters` array in that
//! room's LiveKit metadata.
//!
//! - `GET /generate-stream-link` (signed as scene): the platform ban, then
//!   owner or admin of the bound scene's place; the active credential is
//!   reused or replaced exactly as `POST /scene-stream-access` does.
//! - `GET /stream-info/{streamingKey}` (public): the place behind a key.
//! - `POST /streamer-token`, `POST /presentation-bot-token` (the key is
//!   the credential): publish tokens for the key's room; the streamer is
//!   added to the room's presenters.
//! - `POST /watcher-token` (a viewer's own signed fetch, never a scene's):
//!   the platform ban, then the scene ban of the room the stream runs in
//!   and, for a world, its access policy; a subscribe-only token whose
//!   `dcl.castWatcher` attribute carries the wallet, so the ban and world
//!   access sweeps reach a watcher already in the room, and the join guard
//!   removes one banned since its token ([`refuse_on_join`]; upstream reads
//!   the bans at the token only).
//! - `GET /presenters`, `PUT|DELETE /presenters/{identity}` (signed as
//!   scene): the platform ban, then owner or admin of the place the room's
//!   stream credential is stored for. As upstream, a demotion does not stick for a scene admin:
//!   the next join of that admin adds them back.
//!
//! Publisher identities name the credential that minted them
//! (`stream:<place>:<access id>:<uuid>`, `presentation-bot:<room>:<access
//! id>:<uuid>`; upstream has `stream:<place>:<uuid>`). Revoking or replacing a
//! credential, or removing its place from the catalogue, removes its
//! publishers and their presenter entries, and the `participant_joined`
//! webhook removes one that connects later with a token still inside its
//! five-minute lifetime.
//!
//! A key lives only while the wallet that generated it would still be given
//! it ([`recheck_keys`]): not platform-banned, not denied, and holding the key
//! decision on the room's scene (a world scene's admin through the wallet that
//! granted it). The check runs on every publisher token and stream link,
//! when a scene admin is removed, when a wallet is banned, when PostgreSQL
//! announces a deployment or an authority change of a world ([`listen`]),
//! and in the key job for the losses with no event here. Upstream never reads
//! the minter again: there a key works until it expires or an admin revokes it.
//!
//! A key is for one scene room. A redeploy gives the scene a new entity id
//! and so a new room, and the key made before would feed a room no scene
//! uses: the same check revokes a key whose scene no longer stands in its
//! room ([`still_deployed`]), which ends a stream still running there.
//! Upstream keeps that key until the next stream link or `POST
//! /scene-stream-access` replaces it (409 while it streams): its `GET`
//! answers it and its tokens publish into the old room. A scene admin is
//! granted on the scene's place (`ActiveDeployment::place_key`), as upstream
//! grants on the Places place: a redeploy of the same place keeps the grant.
//!
//! The `streaming` flag follows LiveKit and not only its two webhooks, which
//! upstream applies in arrival order: the streaming job raises the flag of a
//! credential whose ingress publishes and lowers one with no stream, an
//! `ingress_ended` is ignored while LiveKit reports the ingress live, and a
//! stream link asks LiveKit before it replaces an idle credential. So a lost
//! or late webhook does not lift the four-hour cap or the renewal conflict.
//!
//! A display name is shown as asked, without invisible characters, and a name
//! that reads as another wallet's address (in full or abbreviated, through the
//! lookalikes `shown_name` folds) or as another viewer's `Guest#` default is
//! replaced by the caller's own default. Lookalikes outside that fold are not
//! caught.
//!
//! Errors answer upstream's `{"error": msg}` with its statuses; a signature
//! refusal answers the crypto middleware's `{"ok": false, "message"}` and a
//! body refusal `@dcl/schema-validator-component`'s. Local preview realms are
//! not special-cased here, and `ALLOW_LOCAL_PREVIEW` does not apply to Cast
//! (it only skips `/get-scene-adapter`'s deployment check); upstream's Cast
//! skips the place permission check for them when that setting is on.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Path, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use chrono::NaiveDateTime;
use serde::Serialize;
use serde_json::{Map, Value, json};
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use uuid::Uuid;

use super::super::admission::scene::{self, PlaceLookup};
use super::super::auth::{
    SCENE_SIGNER, SignedFetch, device_identifier, verify_signed_fetch, verify_viewer_signed_fetch,
};
use super::super::errors::{ApiError, forbidden, service_unavailable, unauthorized};
use super::super::livekit::{AccessToken, TRACK_SOURCE_MICROPHONE, VideoGrants};
use super::super::{CommsState, SharedState};
use super::{
    Context, FOUR_DAYS_MS, KeyTarget, NOT_AUTHORIZED_TO_STREAM_MSG, active_access, issue_access,
    refuse_barred, resolve_place_id,
};
use crate::access::SceneBanKey;
use crate::rooms::{ParsedRoom, ResolveError, ResolvedRoom, RoomRequest};
use crate::shared::signed_fetch::SignedPath;
use crate::world_identity::{Parcel, WorldName};
use crate::{Address, AuthorityNotEstablished, PermissionDecision};

pub const GENERATE_STREAM_LINK_PATH: &str = "/cast/generate-stream-link";
pub const WATCHER_TOKEN_PATH: &str = "/cast/watcher-token";
pub const PRESENTERS_PATH: &str = "/cast/presenters";

/// `CAST2_URL`'s default, upstream's `buildStreamLinks` fallback.
pub const DEFAULT_CAST2_URL: &str = "https://cast2.decentraland.org";

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

// Upstream's error texts (`logic/cast/errors.ts`, the handlers).
pub const INVALID_STREAMING_KEY_MSG: &str = "Invalid or expired streaming token";
pub const EXPIRED_STREAMING_KEY_MSG: &str = "Streaming token has expired";
pub const EXPIRED_STREAM_ACCESS_MSG: &str =
    "Stream access has expired. Please generate a new stream link.";
pub const NOT_STREAM_LINK_ADMIN_MSG: &str = "Only scene administrators can generate stream links";
/// The refusal of the scene adapter for the same world (`admission::handlers`).
pub const WORLD_ACCESS_DENIED_MSG: &str =
    "Access denied, you are not authorized to access this world";
pub const NOT_PRESENTER_ADMIN_MSG: &str = "Only scene administrators can manage presenters";
pub const SCENE_BANNED_WATCHER_MSG: &str = "You are banned from this scene";
pub const NO_REALM_MSG: &str = "Access denied, invalid signed-fetch request, no realm";
pub const INVALID_PRESENTER_IDENTITY_MSG: &str =
    "participantIdentity must be a valid Ethereum address or streamer identity";
const LIVEKIT_UNCONFIGURED_MSG: &str =
    "LiveKit is not configured (set LIVEKIT_API_KEY / LIVEKIT_API_SECRET)";

fn no_active_stream(location: &str) -> ApiError {
    ApiError::not_found(format!("No active stream found for {location}"))
}

fn require_livekit(state: &CommsState) -> Result<(), ApiError> {
    if state.livekit.configured {
        Ok(())
    } else {
        Err(service_unavailable(LIVEKIT_UNCONFIGURED_MSG))
    }
}

// ── the credential row ──

/// A stream credential as the cast routes read it.
#[derive(sqlx::FromRow)]
struct KeyedAccess {
    id: Uuid,
    place_id: String,
    room_id: Option<String>,
    generated_by: String,
    created_at: NaiveDateTime,
    expiration_time: Option<NaiveDateTime>,
}

/// The stored expiry, else the legacy four days from creation (upstream's
/// `getStreamAccessExpirationTime`).
fn expires_ms(created_at: NaiveDateTime, expiration_time: Option<NaiveDateTime>) -> i64 {
    expiration_time
        .map(|t| t.and_utc().timestamp_millis())
        .unwrap_or_else(|| created_at.and_utc().timestamp_millis() + FOUR_DAYS_MS)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The active credential holding `key` (`scene_stream_access_active_key_idx`).
async fn access_by_key(pool: &PgPool, key: &str) -> Result<Option<KeyedAccess>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, place_id, room_id, generated_by, created_at, expiration_time FROM social.scene_stream_access \
         WHERE streaming_key = $1 AND active ORDER BY created_at DESC LIMIT 1",
    )
    .bind(key)
    .fetch_optional(pool)
    .await
}

/// The active, unexpired credential streaming into `room`
/// (`scene_stream_access_active_room_idx`). An expired row the expiry job has
/// not deactivated yet counts as none.
async fn access_by_room(pool: &PgPool, room: &str) -> Result<Option<KeyedAccess>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT id, place_id, room_id, generated_by, created_at, expiration_time FROM social.scene_stream_access \
         WHERE room_id = $1 AND active \
         AND COALESCE(expiration_time, created_at + interval '4 days') > timezone('UTC', now()) \
         ORDER BY {} LIMIT 1",
        super::ROOM_KEY_ORDER
    )))
    .bind(room)
    .fetch_optional(pool)
    .await
}

/// A key's credential for the publishing tokens: unknown 401, expired 401,
/// without a room 401 (upstream's `validateStreamerToken` and
/// `generatePresentationBotToken`). Returns the place, the room and the
/// credential's id, which the publisher identities carry. A key whose minter
/// lost the place is revoked here and answers as an unknown one.
async fn publishing_access(
    state: &SharedState,
    key: &str,
) -> Result<(String, String, Uuid), ApiError> {
    let access = access_by_key(&state.pool, key)
        .await?
        .ok_or_else(|| unauthorized(INVALID_STREAMING_KEY_MSG))?;
    if now_ms() >= expires_ms(access.created_at, access.expiration_time) {
        return Err(unauthorized(EXPIRED_STREAMING_KEY_MSG));
    }
    if !matches!(keep_or_revoke(state, &access).await?, Checked::Kept) {
        return Err(unauthorized(INVALID_STREAMING_KEY_MSG));
    }
    let room = access
        .room_id
        .ok_or_else(|| unauthorized(INVALID_STREAMING_KEY_MSG))?;
    Ok((access.place_id, room, access.id))
}

/// Why `access` ends now, if it does: the wallet that generated it would not
/// be given the key (the gates of [`generate_stream_link`] on the scene of the
/// credential's room), or that scene no longer stands there
/// ([`still_deployed`]). An authority that cannot be read is an error, never
/// a loss. A key with no recorded minter (an imported one) ends with its
/// scene or at its expiry.
async fn key_ends(
    state: &CommsState,
    access: &KeyedAccess,
) -> Result<Option<&'static str>, ApiError> {
    let minter = Address::parse(&access.generated_by).ok();
    if let Some(minter) = &minter
        && let Some(why) = super::barred(state, minter, None).await?
    {
        return Ok(Some(why));
    }
    let room = access.room_id.as_deref().unwrap_or_default();
    let resolved = room_scene(state, room).await?;
    if !still_deployed(state, room, resolved.as_ref()).await? {
        return Ok(Some(SCENE_REPLACED));
    }
    let Some(minter) = minter else {
        return Ok(None);
    };
    // A world scene's key is decided on that scene, wherever the row is stored.
    let world_scene = resolved.as_ref().and_then(|resolved| {
        let world = resolved.realm.world()?;
        Some((world, resolved.deployment.as_ref()?))
    });
    let decision = match world_scene {
        Some((world, deployment)) => {
            let target = KeyTarget::deployed(state, world, deployment, room.to_owned());
            target.decide(state, &minter).await
        }
        None => {
            let scene = match &resolved {
                Some(resolved) => super::scene_authority(state, resolved, &access.place_id).await?,
                None => None,
            }
            .unwrap_or_else(|| PlaceLookup::new(state, &access.place_id, scene::NO_ADMINS));
            scene::scene_manager_decision(state, &scene, &minter).await
        }
    };
    match decision {
        PermissionDecision::Allowed => Ok(None),
        decision if super::lacks_authority(&decision) => Ok(Some("its minter lost the place")),
        PermissionDecision::Refused(why) => Err(why.into()),
    }
}

/// Whether the scene of a credential's `room` is the one deployed now on its
/// base parcel, as a stream link binds it ([`Context::resolve`]). A redeploy
/// gives the scene a new entity id and so a new room, and the key made before
/// would feed a room no scene uses. That the room resolves is not enough: a
/// world keeps a replaced scene for a week and resolves its room, for the
/// players still in it. A Genesis scene this node does not store counts as
/// standing; one it stores without an active deployment was replaced.
async fn still_deployed(
    state: &CommsState,
    room: &str,
    resolved: Option<&ResolvedRoom>,
) -> Result<bool, ApiError> {
    let Some(ResolvedRoom {
        realm,
        scene_id: Some(scene),
        deployment,
        ..
    }) = resolved
    else {
        // A world scene room resolves to a deployment or to nothing.
        return Ok(resolved.is_some() || !room.starts_with(crate::rooms::SCENE_ROOM_PREFIX));
    };
    // A world room resolves with its deployment; a Genesis room names its scene alone.
    let scenes = &state.scenes;
    let base = match deployment {
        Some(placed) => Ok(Some(placed.base)),
        None => (scenes.resolve(RoomRequest::Adapter { realm, scene }).await)
            .map(|scene| scene.deployment.map(|placed| placed.base)),
    };
    let current = match base {
        Ok(Some(parcel)) => scenes.resolve(RoomRequest::Parcel { realm, parcel }).await,
        Ok(None) => {
            let unknown =
                "SELECT NOT EXISTS (SELECT FROM content.deployments WHERE entity_id = $1)";
            let unknown = sqlx::query_scalar(unknown).bind(scene.as_str());
            return unknown.fetch_one(&state.pool).await.map_err(|error| {
                AuthorityNotEstablished::store_unavailable("content.deployments", &error).into()
            });
        }
        Err(error) => Err(error),
    };
    match current {
        Ok(current) => Ok(current.room == room),
        Err(ResolveError::NoSceneAtParcel { .. }) => Ok(false),
        Err(ResolveError::Db(error)) => {
            Err(AuthorityNotEstablished::store_unavailable("content.deployments", &error).into())
        }
        Err(_) => Ok(true),
    }
}

/// What a recheck did with one key.
enum Checked {
    Kept,
    /// This recheck revoked it.
    Revoked(RevokedKey),
    /// Someone else ended it meanwhile.
    Gone,
}

/// A key a recheck revoked: its room, and whether it ended because its scene
/// was replaced or removed (a redeploy of its place).
pub(crate) struct RevokedKey {
    pub room: Option<String>,
    pub replaced: bool,
}

const SCENE_REPLACED: &str = "its scene was replaced or removed";

/// Revokes `access` when it ends ([`key_ends`]), as `DELETE
/// /scene-stream-access` does: the ingress is deleted, its publishers leave
/// and the place's owner is told. A webhook or the reconciliation can change
/// the row between the snapshot and the write: the revocation is tried
/// again, and is an error while it is not stored.
async fn keep_or_revoke(state: &SharedState, access: &KeyedAccess) -> Result<Checked, ApiError> {
    let Some(why) = key_ends(state, access).await? else {
        return Ok(Checked::Kept);
    };
    for _ in 0..3 {
        let revoke = super::jobs::CleanupCommand::Revoke;
        let Some(revocation) = super::jobs::prepare(state, access.id, revoke).await? else {
            return Ok(Checked::Gone);
        };
        if revocation.record(&state.pool).await? {
            tracing::info!(place_id = %access.place_id, minter = %access.generated_by, why, "stream key revoked");
            super::settle_cleanup(state, access.id, &access.place_id);
            return Ok(Checked::Revoked(RevokedKey {
                room: access.room_id.clone(),
                replaced: why == SCENE_REPLACED,
            }));
        }
    }
    Err(super::concurrent_change())
}

/// The active credentials a recheck reads.
#[derive(Clone, Copy)]
pub(crate) enum Keys<'a> {
    /// The one streaming into this room: an admin of its scene was removed.
    Room(&'a str),
    /// Those this wallet generated: it was banned.
    Minter(&'a str),
    /// The place's: a stream link is about to answer it.
    Place(&'a str),
    /// One page of places after this cursor: the periodic pass. The cursor
    /// moves to each key before its check, so a pass that is cut goes on with
    /// the next key, and returns to the start after the last page.
    After(&'a Mutex<String>),
    /// Every one: notifications were lost.
    All,
    /// That of the Genesis scene this deployment replaced or removed.
    Replaced(&'a str),
    /// Those of the scene rooms that start with this: a world's.
    World(&'a str),
}

/// [`keep_or_revoke`] on each credential `keys` selects; one that cannot be
/// decided keeps its key for the next recheck. Returns the keys this recheck
/// revoked itself.
pub(crate) async fn recheck_keys(state: &SharedState, keys: Keys<'_>) -> Vec<RevokedKey> {
    let after;
    let (filter, value) = match keys {
        Keys::Room(room) => ("room_id = $1", room),
        Keys::Minter(wallet) => ("lower(generated_by) = $1", wallet),
        Keys::Place(place) => ("place_id = $1", place),
        Keys::After(cursor) => {
            after = cursor.lock().unwrap().clone();
            ("place_id > $1 ORDER BY place_id LIMIT 20", after.as_str())
        }
        Keys::All => ("place_id > $1", ""),
        Keys::Replaced(entity) => (
            "split_part(room_id, ':', 3) IN (SELECT $1 UNION ALL SELECT entity_id \
             FROM content.deployments WHERE deleter_deployment = \
             (SELECT id FROM content.deployments WHERE entity_id = $1))",
            entity,
        ),
        Keys::World(rooms) => ("starts_with(room_id, $1)", rooms),
    };
    let rows: Vec<KeyedAccess> = match sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT id, place_id, room_id, generated_by, created_at, expiration_time \
         FROM social.scene_stream_access WHERE active AND {filter}"
    )))
    .bind(value)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => rows,
        Err(error) => {
            tracing::warn!(%error, "stream keys not read for a recheck");
            return Vec::new();
        }
    };
    let mut revoked = Vec::new();
    for access in &rows {
        if let Keys::After(cursor) = keys {
            cursor.lock().unwrap().clone_from(&access.place_id);
        }
        match keep_or_revoke(state, access).await {
            Ok(Checked::Kept | Checked::Gone) => {}
            Ok(Checked::Revoked(key)) => revoked.push(key),
            Err(error) => {
                tracing::warn!(place_id = %access.place_id, %error, "stream key recheck undecided; the key stays");
            }
        }
    }
    if let Keys::After(cursor) = keys
        && rows.len() < 20
    {
        cursor.lock().unwrap().clear();
    }
    revoked
}

/// The check a notification asks for, `wake` being its payload: `scene:<entity
/// id>` (a Genesis deploy or unpublish), `world:<name>` (the world's scenes,
/// owner or grants changed), or empty for every key.
pub(crate) async fn recheck_notified(state: &SharedState, wake: &str) {
    let rooms;
    let keys = match wake.split_once(':') {
        None => Keys::All,
        Some(("scene", entity)) => Keys::Replaced(entity),
        Some((_, world)) => {
            rooms = crate::rooms::world_scene_room_prefix(world);
            Keys::World(&rooms)
        }
    };
    recheck_keys(state, keys).await;
}

/// The channels of the writes that can make a key stale: every deployment
/// row, and a world's scene, owner and grant changes (`schemas/20_worlds.sql`).
const CHANNELS: [&str; 2] = ["new_deployment", "world_authority"];

/// Calls `check` with each `scene:` and `world:` payload announced on
/// [`CHANNELS`], and with an empty one at each subscription: PostgreSQL keeps
/// no notification for a session that is away. The subscription holds one
/// connection of `pool` and is read while a check runs. The payloads that
/// wait are a set: those that repeat during a check are one check. A session
/// that is idle for a minute is asked a statement: one that ended with no
/// error fails there, and the listener subscribes again.
pub(crate) async fn listen<F: Future<Output = ()>>(pool: &PgPool, check: impl Fn(String) -> F) {
    let waiting = Mutex::new(BTreeSet::new());
    let ready = tokio::sync::Notify::new();
    let wake = |payload: &str| {
        waiting.lock().unwrap().insert(payload.to_owned());
        ready.notify_one();
    };
    let subscription = async {
        loop {
            let ended: sqlx::Result<()> = async {
                let mut listener = PgListener::connect_with(pool).await?;
                listener.listen_all(CHANNELS).await?;
                wake("");
                loop {
                    let idle = Duration::from_secs(60);
                    match tokio::time::timeout(idle, listener.try_recv()).await {
                        Ok(message) => {
                            let Some(message) = message? else {
                                return Ok(());
                            };
                            if let Some(("scene" | "world", _)) = message.payload().split_once(':')
                            {
                                wake(message.payload());
                            }
                        }
                        Err(_) => {
                            let ping = sqlx::query("SELECT 1").execute(&mut listener);
                            let ping = tokio::time::timeout(Duration::from_secs(10), ping);
                            ping.await.map_err(|_| sqlx::Error::PoolTimedOut)??;
                        }
                    }
                }
            }
            .await;
            tracing::warn!(
                ?ended,
                "stream key listener disconnected; it subscribes again"
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    };
    let checks = async {
        loop {
            ready.notified().await;
            let batch = std::mem::take(&mut *waiting.lock().unwrap());
            for payload in batch {
                check(payload).await;
            }
        }
    };
    tokio::join!(subscription, checks);
}

/// A publisher identity minted for credential `access_id`:
/// `stream:<place>:<access id>:<uuid>`, `presentation-bot:<room>:<access id>:<uuid>`.
/// The credential id is what lets a revocation tell a revoked key's
/// publishers from the replacement's, whenever they connect.
fn publisher_identity(kind: &str, scope: &str, access_id: Uuid) -> String {
    format!("{kind}:{scope}:{access_id}:{}", timestamped_uuid())
}

/// A random UUID in the version 7 layout: its first 48 bits are the minting
/// time in unix milliseconds, so a presenter id tells how old its token is.
fn timestamped_uuid() -> Uuid {
    let mut bytes = *Uuid::new_v4().as_bytes();
    let ms = (chrono::Utc::now().timestamp_millis().max(0) as u64).to_be_bytes();
    bytes[..6].copy_from_slice(&ms[2..]);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    Uuid::from_bytes(bytes)
}

/// When the identity's token was minted, for an identity whose last segment
/// is a `timestamped_uuid`.
fn minted_at_ms(identity: &str) -> Option<i64> {
    let uuid = Uuid::parse_str(identity.rsplit(':').next()?).ok()?;
    let bytes = uuid.as_bytes();
    if bytes[6] >> 4 != 7 {
        return None;
    }
    let mut ms = [0u8; 8];
    ms[2..].copy_from_slice(&bytes[..6]);
    Some(u64::from_be_bytes(ms) as i64)
}

/// What a participant identity says about the credential that admitted it.
#[derive(Debug, PartialEq, Eq)]
enum Publisher {
    /// Not a Cast publisher of this place and room.
    Other,
    /// A publisher minted before identities carried the credential id.
    Untagged,
    /// A publisher minted for this credential.
    Tagged(Uuid),
}

/// Classifies `identity`, a participant of `room`: a streamer of any place
/// (only keys of `room` mint streamers into it, and a key moved to another
/// place keeps its minting place in the identity), read from the right as
/// [`tagged_access`] reads it, since a preview place id holds colons; or a
/// bot of `room`.
fn classify_publisher(identity: &str, room: &str) -> Publisher {
    if let Some(rest) = identity.strip_prefix("stream:") {
        let Some((head, uuid)) = rest.rsplit_once(':') else {
            return Publisher::Other;
        };
        if Uuid::parse_str(uuid).is_err() {
            return Publisher::Other;
        }
        return match head.rsplit_once(':') {
            Some((place, access)) if !place.is_empty() => match Uuid::parse_str(access) {
                Ok(access) => Publisher::Tagged(access),
                // `stream:<place with colons>:<uuid>`, minted before tagging.
                Err(_) => Publisher::Untagged,
            },
            _ if head.is_empty() => Publisher::Other,
            _ => Publisher::Untagged,
        };
    }
    let Some(rest) = identity
        .strip_prefix("presentation-bot:")
        .and_then(|r| r.strip_prefix(room))
        .and_then(|r| r.strip_prefix(':'))
    else {
        return Publisher::Other;
    };
    match rest.split_once(':') {
        Some((access, tail)) => match (Uuid::parse_str(access), Uuid::parse_str(tail)) {
            (Ok(access), Ok(_)) => Publisher::Tagged(access),
            _ => Publisher::Other,
        },
        None if Uuid::parse_str(rest).is_ok() => Publisher::Untagged,
        None => Publisher::Other,
    }
}

/// The credential a publisher identity names, wherever it joins:
/// `stream:<place>:<access>:<uuid>` or `presentation-bot:<room>:<access>:<uuid>`.
/// `None` for any other identity, untagged ones included.
fn tagged_access(identity: &str) -> Option<Uuid> {
    if !(identity.starts_with("stream:") || identity.starts_with("presentation-bot:")) {
        return None;
    }
    let (head, uuid) = identity.rsplit_once(':')?;
    let (_, access) = head.rsplit_once(':')?;
    Uuid::parse_str(uuid).ok()?;
    Uuid::parse_str(access).ok()
}

// ── places ──

#[derive(sqlx::FromRow)]
struct CastPlace {
    title: Option<String>,
    world: bool,
    world_name: Option<String>,
    base_position: String,
}

/// The mirrored catalogue, then the worlds published here.
async fn load_place(pool: &PgPool, id: &str) -> Result<Option<CastPlace>, sqlx::Error> {
    sqlx::query_as(
        "SELECT title, world, world_name, base_position FROM explore.place WHERE id = $1 \
         UNION ALL \
         SELECT title, world, world_name, base_position FROM explore.place_world_local WHERE id = $1 \
         LIMIT 1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// The place a world scene's key shows: the world's catalogue row (the
/// local one first, as the removal feed reads it), else the world alone.
async fn world_place(pool: &PgPool, world: &WorldName) -> Result<CastPlace, sqlx::Error> {
    let row = sqlx::query_as(
        "SELECT title, world, world_name, base_position FROM explore.place_indexed \
         WHERE world AND lower(world_name) = $1 \
         ORDER BY (raw->>'local' = 'true') DESC NULLS LAST, id LIMIT 1",
    )
    .bind(world.key())
    .fetch_optional(pool)
    .await?;
    Ok(row.unwrap_or_else(|| CastPlace {
        title: None,
        world: true,
        world_name: Some(world.display().to_owned()),
        base_position: String::new(),
    }))
}

/// The place a key stored under `place_id` streaming into `room` shows: a
/// world scene room's world ([`world_place`]), else the catalogue place.
async fn key_place(
    pool: &PgPool,
    place_id: &str,
    room: Option<&str>,
) -> Result<Option<CastPlace>, sqlx::Error> {
    match room.and_then(crate::rooms::decode_world_scene_room) {
        Some((world, _)) => world_place(pool, &world).await.map(Some),
        None => load_place(pool, place_id).await,
    }
}

/// The room of `world`'s only scene with an active, unexpired key; `None`
/// when no scene room or more than one has one.
async fn single_keyed_room(
    pool: &PgPool,
    world: &WorldName,
) -> Result<Option<String>, sqlx::Error> {
    let rooms: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT room_id FROM social.scene_stream_access \
         WHERE active AND starts_with(room_id, $1) \
         AND COALESCE(expiration_time, created_at + interval '4 days') > timezone('UTC', now()) \
         LIMIT 2",
    )
    .bind(crate::rooms::world_scene_room_prefix(world.key()))
    .fetch_all(pool)
    .await?;
    Ok(match <[String; 1]>::try_from(rooms) {
        Ok([room]) => Some(room),
        Err(_) => None,
    })
}

/// The key a watch location reads, with the room of a world scene
/// ([`active_access`]), and the place it shows. A name ending in `.eth` is a
/// world: the scene on `parcel`, or without one the world's only scene (a
/// multi-scene world needs the parcel, as upstream's `fetchWorldSceneId`).
/// Anything else is a Genesis parcel, resolved as the stream link keyed its
/// credential.
async fn watch_place(
    state: &CommsState,
    location: &str,
    parcel: Option<&str>,
) -> Result<(String, Option<String>, CastPlace), ApiError> {
    let pool = &state.pool;
    let id = if location.to_lowercase().ends_with(".eth") {
        let not_found =
            || ApiError::not_found(format!("No world scene place found for world {location}"));
        let world = WorldName::parse(location).map_err(|_| not_found())?;
        let realm = crate::world_identity::RealmId::World(world.clone());
        let resolved = match parcel {
            Some(raw) => {
                let parcel = Parcel::parse(raw).map_err(|_| not_found())?;
                state
                    .scenes
                    .resolve(RoomRequest::Parcel {
                        realm: &realm,
                        parcel,
                    })
                    .await
            }
            None => {
                let scenes = match state.scenes.world_scene_count(&world).await {
                    Ok(scenes) => scenes,
                    Err(ResolveError::Db(error)) => {
                        return Err(super::scene_identity_unavailable(&error));
                    }
                    Err(_) => return Err(not_found()),
                };
                if scenes > 1 {
                    // A `/w/<world>` link handed out before keys were per
                    // scene still reads the world's one key: the only scene
                    // room of the world with an active key.
                    let Some(room) = single_keyed_room(pool, &world).await? else {
                        return Err(ApiError::bad_request(format!(
                            "A parcel is required to resolve a scene in multi-scene world {location}"
                        )));
                    };
                    state
                        .scenes
                        .resolve(RoomRequest::NamedWorldScene { room: &room })
                        .await
                } else {
                    // A world-shaped scene id resolves to the world's active scene.
                    let scene = crate::world_identity::SceneId::parse(world.display())
                        .map_err(|_| not_found())?;
                    state
                        .scenes
                        .resolve(RoomRequest::Adapter {
                            realm: &realm,
                            scene: &scene,
                        })
                        .await
                }
            }
        };
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(ResolveError::Db(error)) => return Err(super::scene_identity_unavailable(&error)),
            Err(_) => return Err(not_found()),
        };
        if resolved.realm != realm {
            return Err(not_found());
        }
        let deployment = resolved.deployment.as_ref().ok_or_else(not_found)?;
        let place = world_place(pool, &world).await?;
        return Ok((deployment.place_key(), Some(resolved.room), place));
    } else {
        let not_found = || ApiError::not_found(format!("No place found with parcel {location}"));
        // Upstream matches the place's `positions` verbatim.
        match Parcel::parse(location) {
            Ok(parcel) if parcel.canonical() == location => resolve_place_id(pool, location, false)
                .await?
                .ok_or_else(not_found)?,
            _ => return Err(not_found()),
        }
    };
    let place = load_place(pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("No place found with id {id}")))?;
    Ok((id, None, place))
}

/// The scene `room` belongs to, with its realm (a world or the shared
/// realm) and the key its scene bans are stored under (a Genesis room's
/// scene id, a world room's deployment base). `None` when the room names no
/// scene this node can resolve.
async fn room_scene(state: &CommsState, room: &str) -> Result<Option<ResolvedRoom>, ApiError> {
    match crate::rooms::parse_room(room) {
        Some(ParsedRoom::Local(resolved)) => Ok(Some(resolved)),
        Some(ParsedRoom::WorldScene(room)) => {
            match state
                .scenes
                .resolve(RoomRequest::NamedWorldScene { room })
                .await
            {
                Ok(resolved) => Ok(Some(resolved)),
                Err(ResolveError::Db(error)) => Err(super::scene_identity_unavailable(&error)),
                Err(_) => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

/// Where the bans of a room live.
enum RoomBans {
    Key(SceneBanKey),
    /// A world lobby: no scene bans.
    Lobby,
    /// A Genesis scene this node does not hold: refused, never its entity id.
    Unresolved,
}

async fn scene_ban_key(state: &CommsState, scene: &ResolvedRoom) -> Result<RoomBans, ApiError> {
    match scene
        .resolve_ban_key(state.scenes.as_ref(), state.settings.allow_local_preview)
        .await
    {
        Ok(Some(key)) => Ok(RoomBans::Key(key)),
        Ok(None) => Ok(RoomBans::Lobby),
        Err(ResolveError::Db(error)) => {
            Err(AuthorityNotEstablished::store_unavailable("content.deployments", &error).into())
        }
        Err(_) => Ok(RoomBans::Unresolved),
    }
}

// ── tokens ──

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ParticipantMetadata<'a> {
    role: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    room_id: Option<&'a str>,
}

/// Upstream's `livekit.generateCredentials` for the cast roles: an identity
/// in the token's own `cast` list publishes any source, any other only the
/// microphone; `name` is the display name; five minutes.
struct Grant<'a> {
    /// Signed participant attributes; the participant cannot change them.
    attributes: Vec<(&'static str, String)>,
    identity: &'a str,
    room: &'a str,
    can_publish: bool,
    can_update_own_metadata: bool,
    all_sources: bool,
    metadata: ParticipantMetadata<'a>,
}

/// The display name a streamer or watcher asked for, as the room shows it.
/// Upstream shows it verbatim. Here characters that render as nothing
/// (controls, Unicode default-ignorables such as direction overrides,
/// zero-width marks, fillers and tags, and the blank braille cell) are
/// dropped, the name is NFKC-normalized and runs of whitespace become one
/// space. A name that reads as a wallet address other than the caller's own,
/// in full or abbreviated (`0x1234…abcd`), or as another viewer's `Guest#`
/// default, becomes the caller's default (`Guest#<last 4>` for a watcher,
/// `Streamer` for a key holder, who has no wallet here); so does a blank one.
/// Lookalike letters are folded for that check (`address_fold`); ones outside
/// the fold are not caught.
pub(crate) fn shown_name(requested: &str, wallet: Option<&Address>) -> String {
    use unicode_normalization::UnicodeNormalization;
    let own_default = wallet.map(|wallet| {
        let hex = wallet.as_str();
        format!("Guest#{}", &hex[hex.len().saturating_sub(4)..])
    });
    let fallback = || own_default.clone().unwrap_or_else(|| "Streamer".to_owned());
    let visible: String = requested
        .nfkc()
        .filter(|c| !c.is_control() && !renders_as_nothing(*c))
        .collect();
    let name = visible.split_whitespace().collect::<Vec<_>>().join(" ");
    let folded = address_fold(&name);
    let own = wallet.map(|w| w.as_str().to_ascii_lowercase());
    let foreign_address = shown_addresses(&folded).any(|(head, tail)| {
        own.as_deref()
            .is_none_or(|own| !(own.starts_with(&head) && own.ends_with(&tail)))
    });
    let foreign_default = shown_guest_suffixes(&folded).any(|suffix| {
        own_default
            .as_deref()
            .is_none_or(|own| !own.to_ascii_lowercase().ends_with(&format!("#{suffix}")))
    });
    if name.is_empty() || foreign_address || foreign_default {
        fallback()
    } else {
        name
    }
}

/// Characters that take no visible space: Unicode's default-ignorable code
/// points, and the blank braille pattern, which renders as a blank cell.
fn renders_as_nothing(c: char) -> bool {
    use icu_properties::CodePointSetData;
    use icu_properties::props::DefaultIgnorableCodePoint;
    c == '\u{2800}' || CodePointSetData::new::<DefaultIgnorableCodePoint>().contains(c)
}

/// `name` lowercased, with the letters that pass for `0`, `x` and the hex
/// digits mapped to them (Cyrillic and Greek lookalikes, `o` for zero) and
/// the ellipsis spelled `...`: what the address checks read.
fn address_fold(name: &str) -> String {
    name.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'о' | 'ο' | 'o' => '0',
            'х' | 'χ' | '×' => 'x',
            'а' | 'α' => 'a',
            'в' | 'β' => 'b',
            'с' | 'ϲ' => 'c',
            'е' | 'ε' => 'e',
            other => other,
        })
        .collect::<String>()
        .replace('…', "...")
}

/// Every wallet address the folded name shows, as the (head, tail) a reader
/// sees: `0x` + 40 hex digits is a full address (tail empty); `0x` + at least
/// 3 hex digits, two or more dots, and at least 3 hex digits is an
/// abbreviated one.
fn shown_addresses(folded: &str) -> impl Iterator<Item = (String, String)> + '_ {
    folded.match_indices("0x").filter_map(|(at, _)| {
        let rest = &folded[at + 2..];
        let head_len = rest.bytes().take_while(u8::is_ascii_hexdigit).count();
        if head_len >= 40 {
            return Some((format!("0x{}", &rest[..40]), String::new()));
        }
        if head_len < 3 {
            return None;
        }
        let after = &rest[head_len..];
        let dots = after.bytes().take_while(|b| *b == b'.').count();
        let tail_len = after[dots..]
            .bytes()
            .take_while(u8::is_ascii_hexdigit)
            .count();
        (dots >= 2 && tail_len >= 3).then(|| {
            (
                format!("0x{}", &rest[..head_len]),
                after[dots..dots + tail_len].to_owned(),
            )
        })
    })
}

/// The four hex digits after every `guest#` the folded name shows.
fn shown_guest_suffixes(folded: &str) -> impl Iterator<Item = &str> {
    folded.match_indices("guest#").filter_map(|(at, _)| {
        let rest = &folded[at + "guest#".len()..];
        (rest.len() >= 4 && rest.as_bytes()[..4].iter().all(u8::is_ascii_hexdigit))
            .then(|| &rest[..4])
    })
}

fn mint(state: &CommsState, grant: Grant<'_>) -> Result<String, ApiError> {
    let grants = VideoGrants {
        room_join: true,
        room: grant.room.to_owned(),
        can_publish: grant.can_publish,
        can_subscribe: true,
        can_publish_data: true,
        can_update_own_metadata: grant.can_update_own_metadata,
        room_list: Some(false),
        can_publish_sources: (!grant.all_sources).then(|| vec![TRACK_SOURCE_MICROPHONE.to_owned()]),
    };
    let mut token = AccessToken::new(
        &state.livekit.api_key,
        &state.livekit.api_secret,
        grant.identity,
        grants,
    )
    .with_metadata(
        serde_json::to_string(&grant.metadata)
            .map_err(|e| ApiError::internal(format!("cast metadata: {e}")))?,
    );
    for (key, value) in grant.attributes {
        token = token.with_attribute(key, value);
    }
    token.name = grant.metadata.display_name.map(str::to_owned);
    token
        .to_jwt()
        .map_err(|e| ApiError::internal(format!("livekit token: {e}")))
}

/// A room write upstream lets fail as its bare 500.
fn room_service_failure(room: &str, error: impl std::fmt::Debug) -> ApiError {
    tracing::error!(room, ?error, "cast presenter update failed");
    ApiError::internal("Internal Server Error")
}

/// Upstream's `addPresenter`: the room is created when missing, so a
/// streamer can be a presenter before anyone joins.
async fn add_presenter(state: &CommsState, room: &str, identity: &str) -> Result<(), ApiError> {
    let rooms = state.room_service();
    rooms
        .ensure_room(room)
        .await
        .map_err(|e| room_service_failure(room, e))?;
    rooms
        .append_to_room_metadata_array(room, PRESENTERS_FIELD, identity)
        .await
        .map_err(|e| room_service_failure(room, e))
}

const PRESENTERS_FIELD: &str = "presenters";

/// How many streamer ids of one place `presenters` keeps. Each streamer
/// token adds one, connected or not.
const STREAMER_PRESENTERS_MAX: usize = 32;

/// A streamer token's lifetime (the token's default TTL): an id older than
/// this that is not in the room can no longer join.
const STREAMER_TOKEN_TTL_MS: i64 = 5 * 60 * 1000;

/// Upstream's `addPresenter` for a streamer id, the one way a streamer id
/// enters `presenters` (a token, a join), in one read and one write of the
/// room's metadata: streamer ids of the place that are not in the room
/// and whose token has expired go first; if the place still has
/// `STREAMER_PRESENTERS_MAX`, the oldest not in the room make way (a token
/// minted moments ago and not yet connected included). Connected streamers
/// always stay.
async fn add_streamer_presenter(
    state: &CommsState,
    room: &str,
    place_id: &str,
    identity: &str,
) -> Result<(), ApiError> {
    let rooms = state.room_service();
    rooms
        .ensure_room(room)
        .await
        .map_err(|e| room_service_failure(room, e))?;
    let connected: std::collections::HashSet<String> = match rooms.list_participants(room).await {
        Ok(participants) => participants.into_iter().map(|p| p.identity).collect(),
        Err(crate::shared::livekit::RoomServiceError::NotFound) => Default::default(),
        Err(e) => return Err(room_service_failure(room, e)),
    };
    let prefix = format!("stream:{place_id}:");
    let now = now_ms();
    rooms
        .rewrite_room_metadata_array(room, PRESENTERS_FIELD, |current| {
            prune_streamers(current, &prefix, &connected, now, identity)
        })
        .await
        .map_err(|e| room_service_failure(room, e))
}

/// `add_streamer_presenter`'s rewrite of `presenters`.
fn prune_streamers(
    current: Vec<String>,
    prefix: &str,
    connected: &std::collections::HashSet<String>,
    now: i64,
    identity: &str,
) -> Vec<String> {
    let idle = |id: &String| id.starts_with(prefix) && !connected.contains(id);
    let mut kept: Vec<String> = current
        .into_iter()
        .filter(|id| {
            !(idle(id) && minted_at_ms(id).is_none_or(|at| now - at > STREAMER_TOKEN_TTL_MS))
        })
        .collect();
    let streamers = kept.iter().filter(|id| id.starts_with(prefix)).count();
    let mut excess = (streamers + 1).saturating_sub(STREAMER_PRESENTERS_MAX);
    kept.retain(|id| {
        if excess > 0 && idle(id) {
            excess -= 1;
            false
        } else {
            true
        }
    });
    if !kept.iter().any(|id| id == identity) {
        kept.push(identity.to_owned());
    }
    kept
}

/// The `participant_left` webhook: a streamer that left its room leaves
/// `presenters`. The caller skips a leave LiveKit reports for a session the
/// same identity replaced (`DUPLICATE_IDENTITY`), and `on_publisher_joined`
/// puts back a streamer that joins again with a live credential, so a leave
/// that arrives after the rejoin does not drop it for good.
/// Returns whether the participant was a Cast streamer.
pub async fn forget_departed_streamer(
    state: &CommsState,
    room: &str,
    identity: &str,
) -> Result<bool, ApiError> {
    if !state.livekit.configured || !identity.starts_with("stream:") {
        return Ok(false);
    }
    state
        .room_service()
        .remove_from_room_metadata_array(room, PRESENTERS_FIELD, identity)
        .await
        .map_err(|e| ApiError::internal(format!("cast presenters: {e}")))?;
    Ok(true)
}

// ── request bodies ──

/// One string property of an upstream cast body schema:
/// `{type: 'string', pattern: '^\S+.*$', maxLength: 256}`.
struct Field {
    name: &'static str,
    required: bool,
}

const STREAMER_BODY: &[Field] = &[
    Field {
        name: "token",
        required: true,
    },
    Field {
        name: "identity",
        required: true,
    },
];
const WATCHER_BODY: &[Field] = &[
    Field {
        name: "location",
        required: true,
    },
    Field {
        name: "identity",
        required: true,
    },
    Field {
        name: "parcel",
        required: false,
    },
];
const BOT_BODY: &[Field] = &[Field {
    name: "streamingKey",
    required: true,
}];

const BODY_PATTERN: &str = r"^\S+.*$";
const BODY_MAX_LENGTH: usize = 256;

/// ECMAScript `\s`.
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// `^\S+.*$` under the `u` flag: a non-space first character and no line
/// terminator (`.` matches none, `$` is the end of input).
fn matches_body_pattern(s: &str) -> bool {
    s.chars().next().is_some_and(|c| !is_js_space(c))
        && !s.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
}

/// The first AJV error (`allErrors: false`) of an upstream cast body schema.
fn body_schema_error(body: &Value, fields: &[Field]) -> Option<Value> {
    let Some(object) = body.as_object() else {
        return Some(json!({
            "instancePath": "", "schemaPath": "#/type", "keyword": "type",
            "params": { "type": "object" }, "message": "must be object",
        }));
    };
    if let Some(missing) = fields
        .iter()
        .find(|f| f.required && !object.contains_key(f.name))
    {
        return Some(json!({
            "instancePath": "", "schemaPath": "#/required", "keyword": "required",
            "params": { "missingProperty": missing.name },
            "message": format!("must have required property '{}'", missing.name),
        }));
    }
    for field in fields {
        let Some(value) = object.get(field.name) else {
            continue;
        };
        let at = |keyword: &str, params: Value, message: String| {
            json!({
                "instancePath": format!("/{}", field.name),
                "schemaPath": format!("#/properties/{}/{keyword}", field.name),
                "keyword": keyword, "params": params, "message": message,
            })
        };
        let Some(s) = value.as_str() else {
            return Some(at(
                "type",
                json!({ "type": "string" }),
                "must be string".into(),
            ));
        };
        // AJV counts code points.
        if s.chars().count() > BODY_MAX_LENGTH {
            return Some(at(
                "maxLength",
                json!({ "limit": BODY_MAX_LENGTH }),
                format!("must NOT have more than {BODY_MAX_LENGTH} characters"),
            ));
        }
        if !matches_body_pattern(s) {
            return Some(at(
                "pattern",
                json!({ "pattern": BODY_PATTERN }),
                format!("must match pattern \"{BODY_PATTERN}\""),
            ));
        }
    }
    None
}

fn validator_refusal(status: StatusCode, body: &Value) -> Response {
    (
        status,
        [(CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        crate::content::validation::ordered_refusal_json(body),
    )
        .into_response()
}

/// `withSchemaValidatorMiddleware`: a JSON content type (415), a JSON body
/// (400 with the parse error) matching the schema (400 `Invalid JSON body`).
fn validated_body(
    headers: &HeaderMap,
    bytes: &[u8],
    fields: &[Field],
) -> Result<Map<String, Value>, Response> {
    let content_type = headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok());
    if !crate::content::validation::is_json_content_type(content_type) {
        return Err(validator_refusal(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            &json!({ "ok": false, "message": "Content-Type must be application/json" }),
        ));
    }
    let body: Value = serde_json::from_slice(bytes).map_err(|e| {
        validator_refusal(
            StatusCode::BAD_REQUEST,
            &json!({ "ok": false, "message": crate::content::validation::json_parse_message(bytes, &e) }),
        )
    })?;
    if let Some(error) = body_schema_error(&body, fields) {
        return Err(validator_refusal(
            StatusCode::BAD_REQUEST,
            &json!({ "ok": false, "message": "Invalid JSON body", "data": [error] }),
        ));
    }
    match body {
        Value::Object(map) => Ok(map),
        _ => unreachable!("the schema admits only objects"),
    }
}

fn text<'a>(body: &'a Map<String, Value>, field: &str) -> Option<&'a str> {
    body.get(field).and_then(Value::as_str)
}

// ── signed scene requests ──

/// Upstream's `validate`: the signed metadata must carry a `realm` (401)
/// and, for every cast route, a `sceneId` (400).
fn require_scene_metadata(sf: &SignedFetch, missing_scene: &str) -> Result<(), ApiError> {
    if sf.metadata.get("realm").is_none_or(Value::is_null) {
        return Err(unauthorized(NO_REALM_MSG));
    }
    let scene = sf.metadata.get("sceneId");
    if scene.is_none_or(|v| v.is_null() || v.as_str() == Some("")) {
        return Err(ApiError::bad_request(missing_scene));
    }
    Ok(())
}

async fn scene_signed(
    state: &CommsState,
    headers: &HeaderMap,
    method: &str,
    path: SignedPath,
    missing_scene: &str,
) -> Result<SignedFetch, ApiError> {
    let sf = verify_signed_fetch(headers, method, &path, &[SCENE_SIGNER], state.eip1654()).await?;
    require_scene_metadata(&sf, missing_scene)?;
    Ok(sf)
}

// ── GET /cast/generate-stream-link ──

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamLink {
    pub stream_link: String,
    pub watcher_link: String,
    pub streaming_key: String,
    pub place_id: String,
    pub place_name: String,
    pub expires_at: String,
    pub expires_in_days: i64,
}

/// Upstream's `buildStreamLinks`.
pub fn build_stream_links(base: &str, streaming_key: &str, location: &str) -> (String, String) {
    let base = if base.is_empty() {
        DEFAULT_CAST2_URL
    } else {
        base
    };
    (
        format!("{base}/s/{streaming_key}"),
        format!("{base}/w/{location}"),
    )
}

/// `Date.prototype.toISOString` of a millisecond instant.
fn iso_millis(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// `Math.ceil` of the days left.
fn days_left(expires_ms: i64, now_ms: i64) -> i64 {
    (expires_ms - now_ms + DAY_MS - 1).div_euclid(DAY_MS)
}

pub async fn generate_stream_link(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let sf = scene_signed(
        &state,
        &headers,
        "get",
        SignedPath::at(&uri, GENERATE_STREAM_LINK_PATH),
        "sceneId is required in authMetadata for Cast2 chat functionality",
    )
    .await?;
    require_livekit(&state)?;
    // Before the admin lookup, so the refusal is no admin-status oracle.
    let device_id = device_identifier(&sf.metadata);
    let denied = forbidden(NOT_AUTHORIZED_TO_STREAM_MSG);
    refuse_barred(&state, &sf.signer, device_id.as_deref(), denied).await?;
    // The room, the key and the authority all come from the one bound scene.
    let bound = Context::resolve(&state, &sf.metadata).await?;
    if let Some((world, _)) = &bound.world_scene {
        super::require_world_place(&state.pool, world).await?;
    }
    let world_base = bound.world_scene.as_ref().map(|(world, _)| {
        let base = bound.footprint.base_position.clone().unwrap_or_default();
        (world.clone(), base)
    });
    let target = KeyTarget::bound(&state, bound).await?;
    let place_id = target.place_id.clone();
    match target.decide(&state, &sf.signer).await {
        PermissionDecision::Allowed => {}
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
            tracing::warn!(signer = %sf.signer, place_id, "stream link refused: not a scene admin");
            return Err(forbidden(NOT_STREAM_LINK_ADMIN_MSG));
        }
        PermissionDecision::Refused(why) => return Err(why.into()),
    }
    let key = (place_id.as_str(), target.world_room.as_deref());
    // A key whose minter lost the place is never the one answered.
    super::recheck_target(&state, key.0, key.1).await;
    let access = issue_access(
        &state,
        &state.ingress(),
        key,
        &target.room,
        &sf.signer,
        &target.admin_place_key,
    )
    .await?;
    let row = key_place(&state.pool, &place_id, Some(&target.room)).await?;
    let (title, location) = match &row {
        Some(row) => (
            row.title.clone().filter(|t| !t.is_empty()),
            row.world_name
                .clone()
                .filter(|w| !w.is_empty())
                .unwrap_or_else(|| row.base_position.clone()),
        ),
        None => (None, "none".to_owned()),
    };
    // A world with several scenes needs the scene's parcel to watch it
    // (upstream cast2's `/w/<world>/parcel/<parcel>`); a lookup that fails
    // keeps the parcel, which every world accepts.
    let location = match &world_base {
        Some((world, _))
            if state
                .scenes
                .world_scene_count(world)
                .await
                .is_ok_and(|n| n < 2) =>
        {
            location
        }
        Some((_, base)) => format!("{location}/parcel/{base}"),
        None => location,
    };
    let (stream_link, watcher_link) =
        build_stream_links(&state.settings.cast2_url, &access.streaming_key, &location);
    Ok(Json(StreamLink {
        stream_link,
        watcher_link,
        streaming_key: access.streaming_key,
        place_name: title.unwrap_or_else(|| place_id.clone()),
        place_id,
        expires_at: iso_millis(access.ends_at),
        expires_in_days: days_left(access.ends_at, now_ms()),
    })
    .into_response())
}

// ── GET /cast/stream-info/{streamingKey} ──

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamInfo {
    pub place_name: String,
    pub place_id: String,
    pub location: String,
    pub is_world: bool,
}

pub async fn stream_info(
    State(state): State<SharedState>,
    Path(streaming_key): Path<String>,
) -> Result<Json<StreamInfo>, ApiError> {
    if streaming_key.is_empty() {
        return Err(ApiError::bad_request("Streaming key is required"));
    }
    let access = access_by_key(&state.pool, &streaming_key)
        .await?
        .ok_or_else(|| ApiError::bad_request("Invalid streaming key"))?;
    if expires_ms(access.created_at, access.expiration_time) <= now_ms() {
        return Err(ApiError::bad_request("Stream access has expired"));
    }
    if access.room_id.is_none() {
        return Err(ApiError::bad_request(
            "Stream access is missing room information",
        ));
    }
    let place = key_place(&state.pool, &access.place_id, access.room_id.as_deref())
        .await?
        .ok_or_else(|| ApiError::bad_request("Place not found"))?;
    let world_name = place.world_name.filter(|w| !w.is_empty());
    let location = if place.world {
        world_name
            .clone()
            .unwrap_or_else(|| place.base_position.clone())
    } else {
        place.base_position.clone()
    };
    Ok(Json(StreamInfo {
        place_name: world_name.unwrap_or(place.base_position),
        place_id: access.place_id,
        location,
        is_world: place.world,
    }))
}

// ── POST /cast/streamer-token ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Credentials {
    pub url: String,
    pub token: String,
    pub room_id: String,
    pub identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place_name: Option<String>,
}

pub async fn streamer_token(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let body = match validated_body(&headers, &body, STREAMER_BODY) {
        Ok(body) => body,
        Err(refusal) => return Ok(refusal),
    };
    let key = text(&body, "token").unwrap_or_default();
    let display_name = shown_name(text(&body, "identity").unwrap_or_default(), None);
    let display_name = display_name.as_str();
    let (place_id, room, access_id) = publishing_access(&state, key).await?;
    // The streamer is registered as a presenter in the room's metadata.
    require_livekit(&state)?;
    // A unique LiveKit identity naming the credential; the display name rides
    // in the metadata.
    let identity = publisher_identity("stream", &place_id, access_id);
    let token = mint(
        &state,
        Grant {
            attributes: Vec::new(),
            identity: &identity,
            room: &room,
            can_publish: true,
            can_update_own_metadata: true,
            all_sources: true,
            metadata: ParticipantMetadata {
                role: "streamer",
                display_name: Some(display_name),
                room_id: None,
            },
        },
    )?;
    add_streamer_presenter(&state, &room, &place_id, &identity).await?;
    tracing::info!(room, place_id, "cast streamer token issued");
    Ok(Json(Credentials {
        url: state.livekit.client_url.clone(),
        token,
        room_id: room,
        identity,
        place_name: None,
    })
    .into_response())
}

// ── POST /cast/watcher-token ──

pub async fn watcher_token(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    // A viewer's own wallet, so the scene's bans reach it; never a scene.
    let sf = verify_viewer_signed_fetch(
        &headers,
        "post",
        &SignedPath::at(&uri, WATCHER_TOKEN_PATH),
        state.eip1654(),
    )
    .await?;
    let body = match validated_body(&headers, &body, WATCHER_BODY) {
        Ok(body) => body,
        Err(refusal) => return Ok(refusal),
    };
    let location = text(&body, "location").unwrap_or_default();
    let watcher = &sf.signer;
    let display_name = shown_name(text(&body, "identity").unwrap_or_default(), Some(watcher));
    let display_name = display_name.as_str();
    // Before the location, so an unresolvable place still refuses.
    let device_id = device_identifier(&sf.metadata);
    let denied = unauthorized(super::super::voice::DENY_LISTED_MSG);
    refuse_barred(&state, watcher, device_id.as_deref(), denied).await?;
    let parcel = text(&body, "parcel").filter(|parcel| !parcel.is_empty());
    let (place_id, world_room, place) = watch_place(&state, location, parcel).await?;
    let access = active_access(&state.pool, &place_id, world_room.as_deref())
        .await?
        .ok_or_else(|| no_active_stream(location))?;
    if now_ms() >= expires_ms(access.created_at, access.expiration_time) {
        return Err(unauthorized(EXPIRED_STREAM_ACCESS_MSG));
    }
    let room = access.room_id.ok_or_else(|| no_active_stream(location))?;
    // Watchers join the scene's own room: its scene bans hold for them too,
    // and so does a world's access policy (allow-list, password, NFT gate,
    // world bans), which every other join of that room passes.
    let scene_room = room_scene(&state, &room)
        .await?
        .ok_or_else(|| no_active_stream(location))?;
    let RoomBans::Key(ban_key) = scene_ban_key(&state, &scene_room).await? else {
        return Err(no_active_stream(location));
    };
    if is_scene_banned(&state, &ban_key, watcher).await? {
        tracing::warn!(%watcher, place_id, "cast watcher token refused: scene ban");
        return Err(forbidden(SCENE_BANNED_WATCHER_MSG));
    }
    let identity = format!("watch:{room}:{}", Uuid::new_v4());
    // A world watcher carries the authorization proof a scene join carries
    // (`admission::handlers`), stamped before the access check: the world
    // access sweep keeps a watcher admitted after the world's last access
    // edit (a password rotation included) and removes the rest.
    let mut world_proof = None;
    if let Some(world) = scene_room.realm.world() {
        let started =
            super::super::admission::world_mint::authorization_started(&state.pool, world)
                .await
                .map_err(|error| {
                    tracing::error!(%error, "world authorization unavailable");
                    ApiError::http(503, "World authorization unavailable")
                })?;
        let caller_ip = crate::shared::client_ip::client_ip(&headers);
        let caller = if caller_ip.is_empty() {
            watcher.as_str()
        } else {
            &caller_ip
        };
        let secret = scene::meta_str(&sf.metadata, "secret");
        let allowed = scene::has_world_access_permission(
            &state,
            watcher,
            world.key(),
            scene_room.scene_id.as_ref().map(|s| s.as_str()),
            secret.as_deref(),
            device_id.as_deref(),
            caller,
        )
        .await?;
        if !allowed {
            tracing::warn!(%watcher, place_id, "cast watcher token refused: world access");
            return Err(unauthorized(WORLD_ACCESS_DENIED_MSG));
        }
        let proof = super::super::admission::world_mint::authorization_proof(
            &state.livekit.api_secret,
            world,
            &identity,
            &room,
            started,
        );
        world_proof = Some(
            json!({ super::super::admission::world_mint::AUTHORIZATION_FIELD: proof }).to_string(),
        );
    }
    let mut attributes = vec![(WATCHER_WALLET_ATTRIBUTE, watcher.as_str().to_owned())];
    if let Some(proof) = world_proof {
        attributes.push((
            super::super::admission::world_mint::AUTHORIZATION_ATTRIBUTE,
            proof,
        ));
    }
    let token = mint(
        &state,
        Grant {
            attributes,
            identity: &identity,
            room: &room,
            can_publish: false,
            // A watcher cannot rewrite its own role.
            can_update_own_metadata: false,
            all_sources: false,
            metadata: ParticipantMetadata {
                role: "watcher",
                display_name: Some(display_name),
                room_id: Some(&room),
            },
        },
    )?;
    let place_name = place
        .title
        .filter(|t| !t.is_empty())
        .or(place.world_name.filter(|w| !w.is_empty()))
        .unwrap_or_else(|| location.to_owned());
    Ok(Json(Credentials {
        url: state.livekit.client_url.clone(),
        token,
        room_id: room,
        identity,
        place_name: Some(place_name),
    })
    .into_response())
}

async fn is_scene_banned(
    state: &CommsState,
    key: &SceneBanKey,
    watcher: &Address,
) -> Result<bool, ApiError> {
    state
        .moderation
        .is_scene_banned(key.as_str(), watcher)
        .await
        .map_err(|e| AuthorityNotEstablished::store_unavailable("social.scene_bans", &e).into())
}

// ── POST /cast/presentation-bot-token ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BotCredentials {
    pub url: String,
    pub token: String,
    pub room_id: String,
}

pub async fn presentation_bot_token(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let body = match validated_body(&headers, &body, BOT_BODY) {
        Ok(body) => body,
        Err(refusal) => return Ok(refusal),
    };
    let key = text(&body, "streamingKey").unwrap_or_default();
    let (_place_id, room, access_id) = publishing_access(&state, key).await?;
    let identity = publisher_identity("presentation-bot", &room, access_id);
    let token = mint(
        &state,
        Grant {
            attributes: Vec::new(),
            identity: &identity,
            room: &room,
            can_publish: true,
            // It publishes its layout state in its own metadata.
            can_update_own_metadata: true,
            all_sources: true,
            metadata: ParticipantMetadata {
                role: "presentation",
                display_name: None,
                room_id: None,
            },
        },
    )?;
    Ok(Json(BotCredentials {
        url: state.livekit.client_url.clone(),
        token,
        room_id: room,
    })
    .into_response())
}

// ── watchers ──

/// The attribute of a watcher's token that holds the viewer's wallet.
pub(crate) const WATCHER_WALLET_ATTRIBUTE: &str = "dcl.castWatcher";

/// The wallet a Cast watcher (`watch:<room>:<uuid>`) signed in with, from
/// the attribute its token set: its identity is synthetic, so the ban and
/// world access sweeps read it here. `None` for any other participant.
pub(crate) fn watcher_wallet(
    identity: &str,
    attributes: &std::collections::BTreeMap<String, String>,
) -> Option<Address> {
    if !identity.starts_with("watch:") {
        return None;
    }
    attributes
        .get(WATCHER_WALLET_ATTRIBUTE)
        .and_then(|wallet| Address::parse(wallet).ok())
}

// ── revoking a credential ──

/// The Cast publishers a revoked or replaced credential admitted, removed
/// from the room and from its `presenters`. The streaming key mints WebRTC
/// publishers, which deleting the RTMP ingress does not stop. Each publisher
/// identity names the credential that minted it (`publisher_identity`), so
/// only `keep`'s publishers stay (the credential that replaced the revoked
/// one in this room, if any), however late they connect. Untagged
/// publishers, minted before identities carried the credential, always go.
/// Idempotent; the durable cleanup retries it, and `refuse_revoked_publisher`
/// removes a revoked publisher that connects later.
pub(crate) async fn evict_revoked_publishers(
    state: &CommsState,
    room: &str,
    keep: Option<Uuid>,
) -> Result<(), ApiError> {
    if !state.livekit.configured {
        return Ok(());
    }
    let rooms = state.room_service();
    let revoked = |identity: &str| match classify_publisher(identity, room) {
        Publisher::Other => false,
        Publisher::Untagged => true,
        Publisher::Tagged(access) => Some(access) != keep,
    };
    let participants = match rooms.list_participants(room).await {
        Ok(participants) => participants,
        Err(crate::shared::livekit::RoomServiceError::NotFound) => Vec::new(),
        Err(e) => return Err(ApiError::internal(format!("cast eviction: {e}"))),
    };
    for p in participants.iter().filter(|p| revoked(&p.identity)) {
        rooms
            .remove_participant(room, &p.identity)
            .await
            .map_err(|e| ApiError::internal(format!("cast eviction: {e}")))?;
        tracing::info!(room, identity = %p.identity, "cast publisher of a revoked credential removed");
    }
    let presenters: Vec<String> = match rooms.get_room_info(room).await {
        Ok(Some(info)) => {
            let metadata = crate::shared::livekit::parse_room_metadata(
                info.get("metadata").and_then(|m| m.as_str()),
            );
            metadata
                .get(PRESENTERS_FIELD)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        }
        Ok(None) | Err(crate::shared::livekit::RoomServiceError::NotFound) => Vec::new(),
        Err(e) => return Err(ApiError::internal(format!("cast eviction: {e}"))),
    };
    for id in presenters.iter().filter(|id| revoked(id)) {
        rooms
            .remove_from_room_metadata_array(room, PRESENTERS_FIELD, id)
            .await
            .map_err(|e| ApiError::internal(format!("cast eviction: {e}")))?;
    }
    Ok(())
}

/// The `participant_joined` guard: a Cast publisher whose credential is no
/// longer the active one for `room` is removed as it connects. A publisher
/// token lives five minutes and LiveKit honours it until then, so a revoked
/// key's holder could otherwise join after the revocation's eviction pass.
/// Untagged publishers (minted before identities carried the credential) are
/// removed when the room has no active credential. Returns whether it removed
/// the participant.
/// The `participant_joined` webhook for Cast publishers: a revoked key's
/// publisher is removed (`refuse_revoked_publisher`); a streamer of the live
/// credential is (again) a presenter, which a full reconnect's leave of the
/// old session may have removed. Only a failed guard is an error.
pub async fn on_publisher_joined(
    state: &CommsState,
    room: &str,
    identity: &str,
    watcher: Option<&Address>,
) -> Result<(), ApiError> {
    if refuse_on_join(state, room, identity, watcher).await? || !identity.starts_with("stream:") {
        return Ok(());
    }
    if !state.livekit.configured {
        return Ok(());
    }
    // The guard let it in, so the room has an active credential.
    let access = access_by_room(&state.pool, room).await;
    let access = access.inspect_err(
        |error| tracing::warn!(room, %error, "cast credential not read for a streamer's join"),
    );
    let Ok(Some(access)) = access else {
        return Ok(());
    };
    if let Err(error) = add_streamer_presenter(state, room, &access.place_id, identity).await {
        tracing::warn!(room, %error, "cast presenters not updated on join");
    }
    Ok(())
}

/// The join guards of `participant_joined`: a publisher of a revoked key
/// ([`refuse_revoked_publisher`]), and a `watcher` (its wallet, from the
/// token's attribute) banned or deny-listed since its token ([`super::barred`]),
/// or banned from the room's scene. The removal of a banned wallet reads each room once, and
/// the token lives five minutes more. A world's access is the access sweep's.
/// Returns whether it removed the participant.
pub async fn refuse_on_join(
    state: &CommsState,
    room: &str,
    identity: &str,
    watcher: Option<&Address>,
) -> Result<bool, ApiError> {
    let Some(wallet) = watcher.filter(|_| state.livekit.configured) else {
        return refuse_revoked_publisher(state, room, identity).await;
    };
    let banned = super::barred(state, wallet, None).await?.is_some()
        || match room_scene(state, room).await? {
            Some(scene) => match scene_ban_key(state, &scene).await? {
                RoomBans::Key(key) => is_scene_banned(state, &key, wallet).await?,
                RoomBans::Lobby => false,
                RoomBans::Unresolved => true,
            },
            None => false,
        };
    if banned {
        state
            .room_service()
            .remove_participant(room, identity)
            .await
            .map_err(|e| ApiError::internal(format!("cast join guard: {e}")))?;
        tracing::info!(room, identity, "banned cast watcher refused on join");
    }
    Ok(banned)
}

/// Whether `identity`, a `stream:` or `presentation-bot:` publisher of
/// `room`, belongs to the room's active Cast credential: the credential it is
/// tagged with is the active one, or, untagged, the room still streams.
pub async fn publisher_credential_current(
    pool: &PgPool,
    room: &str,
    identity: &str,
) -> Result<bool, sqlx::Error> {
    let active = access_by_room(pool, room).await?;
    Ok(match (tagged_access(identity), &active) {
        (Some(access), Some(active)) => access == active.id,
        (Some(_), None) => false,
        // Untagged: kept while the room still streams, as before tagging.
        (None, active) => active.is_some(),
    })
}

pub async fn refuse_revoked_publisher(
    state: &CommsState,
    room: &str,
    identity: &str,
) -> Result<bool, ApiError> {
    if !state.livekit.configured
        || !(identity.starts_with("stream:") || identity.starts_with("presentation-bot:"))
    {
        return Ok(false);
    }
    if publisher_credential_current(&state.pool, room, identity).await? {
        return Ok(false);
    }
    state
        .room_service()
        .remove_participant(room, identity)
        .await
        .map_err(|e| ApiError::internal(format!("cast join guard: {e}")))?;
    tracing::info!(
        room,
        identity,
        "cast publisher of a revoked credential refused on join"
    );
    Ok(true)
}

// ── scene admins on join ──

/// Upstream's join hook (`comms-scene-handler.ts`, `cast.addPresenter` for
/// `isSceneOwnerOrAdmin`): a scene owner or admin who joins a scene room is
/// added to its presenters. Here only a room with an active Cast credential
/// is looked at (one indexed read; the admin decision runs for those joins
/// alone), and the caller runs it off the response: like upstream, a failure
/// never refuses the join.
pub async fn add_admin_presenter_on_join(
    state: &CommsState,
    room: &str,
    identity: &crate::Address,
) -> Result<bool, ApiError> {
    if !state.livekit.configured {
        return Ok(false);
    }
    let Some(access) = access_by_room(&state.pool, room).await? else {
        return Ok(false);
    };
    // The room's own scene: explicit admins are granted on its place.
    let place = match room_scene(state, room).await? {
        Some(resolved) => super::scene_authority(state, &resolved, &access.place_id).await?,
        None => None,
    }
    .unwrap_or_else(|| PlaceLookup::new(state, &access.place_id, scene::NO_ADMINS));
    match scene::scene_manager_decision(state, &place, identity).await {
        PermissionDecision::Allowed => {
            add_presenter(state, room, identity.as_str()).await?;
            Ok(true)
        }
        PermissionDecision::Refused(_) => Ok(false),
    }
}

/// At most this many admin lookups run at once; a join past it skips the
/// lookup (a missed promotion, never a refused join).
const ADMIN_LOOKUP_PERMITS: usize = 8;
/// One admin lookup's bound: the DB reads plus the LAND or world owner lookup.
const ADMIN_LOOKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
/// A (room, wallet) looked at recently is not looked at again for this long.
const ADMIN_LOOKUP_TTL: std::time::Duration = std::time::Duration::from_secs(60);
/// The recent-lookup memory is cleared past this many entries.
const ADMIN_LOOKUP_MEMORY: usize = 4096;

fn admin_lookup_permits() -> &'static std::sync::Arc<tokio::sync::Semaphore> {
    static PERMITS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    PERMITS.get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(ADMIN_LOOKUP_PERMITS)))
}

/// Whether (room, wallet) was looked at within `ADMIN_LOOKUP_TTL`; records it
/// when not.
fn admin_lookup_recent(room: &str, identity: &Address) -> bool {
    static RECENT: std::sync::OnceLock<
        parking_lot::Mutex<std::collections::HashMap<(String, String), std::time::Instant>>,
    > = std::sync::OnceLock::new();
    let mut recent = RECENT.get_or_init(Default::default).lock();
    let now = std::time::Instant::now();
    let key = (room.to_owned(), identity.as_str().to_owned());
    if recent
        .get(&key)
        .is_some_and(|at| now.duration_since(*at) < ADMIN_LOOKUP_TTL)
    {
        return true;
    }
    if recent.len() >= ADMIN_LOOKUP_MEMORY {
        recent.retain(|_, at| now.duration_since(*at) < ADMIN_LOOKUP_TTL);
        if recent.len() >= ADMIN_LOOKUP_MEMORY {
            recent.clear();
        }
    }
    recent.insert(key, now);
    false
}

/// Runs `add_admin_presenter_on_join` off the join's response, bounded: a
/// (room, wallet) seen in the last minute is skipped, at most
/// `ADMIN_LOOKUP_PERMITS` run at once (a join past them skips the lookup)
/// and each is cut at `ADMIN_LOOKUP_TIMEOUT`. Throwaway wallets joining a
/// live cast cannot pile up lookups this way.
pub fn spawn_admin_presenter_on_join(state: SharedState, room: String, identity: Address) {
    if !state.livekit.configured || admin_lookup_recent(&room, &identity) {
        return;
    }
    let Ok(permit) = admin_lookup_permits().clone().try_acquire_owned() else {
        tracing::debug!(
            room,
            "cast: admin presenter lookup skipped, all permits in use"
        );
        return;
    };
    crate::shared::task::spawn(async move {
        let _permit = permit;
        match tokio::time::timeout(
            ADMIN_LOOKUP_TIMEOUT,
            add_admin_presenter_on_join(&state, &room, &identity),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => {
                tracing::warn!(
                    room,
                    ?error,
                    "cast: adding a scene admin as presenter failed"
                )
            }
            Err(_) => tracing::warn!(room, "cast: admin presenter lookup timed out"),
        }
    });
}

// ── /cast/presenters ──

/// Upstream's `isValidPresenterIdentity`: a wallet, or a streamer identity
/// this module mints (`stream:<place id>:<uuid>`).
pub fn is_valid_presenter_identity(identity: &str) -> bool {
    if identity.len() > 512 {
        return false;
    }
    if let Some(hex) = identity.strip_prefix("0x") {
        return hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    let Some((place, uuid)) = identity
        .strip_prefix("stream:")
        .and_then(|rest| rest.rsplit_once(':'))
    else {
        return false;
    };
    let place_ok = !place.is_empty()
        && place
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:+/=-".contains(&b));
    let uuid_ok = uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        });
    place_ok && uuid_ok
}

/// The signed scene's room, once the signer owns or administers the place
/// the room's active stream credential is stored for (upstream's
/// `validatePresenterAdmin`): no stream 404, a vanished place 404, anyone
/// else 403. A platform-banned or denied signer is refused first, as on the
/// stream link: a banned wallet holds no Cast authority (upstream has no ban
/// gate here).
async fn presenter_room(state: &CommsState, sf: &SignedFetch) -> Result<String, ApiError> {
    require_scene_metadata(sf, "sceneId is required in authMetadata")?;
    require_livekit(state)?;
    let device_id = device_identifier(&sf.metadata);
    let denied = forbidden(NOT_PRESENTER_ADMIN_MSG);
    refuse_barred(state, &sf.signer, device_id.as_deref(), denied).await?;
    let bound = Context::resolve(state, &sf.metadata).await?;
    let room = bound.room;
    let access = access_by_room(&state.pool, &room)
        .await?
        .ok_or_else(|| no_active_stream(&room))?;
    // A Genesis key's catalogue place must still exist; a world scene's key
    // is stored under its scene's place, which is no catalogue row, and needs
    // its world's.
    let stored = PlaceLookup::new(state, &access.place_id, scene::NO_ADMINS);
    let stored = match &bound.world_scene {
        Some((world, _)) => match super::world_catalogued(&state.pool, world).await {
            Ok(true) => Ok(Some(&bound.footprint)),
            Ok(false) => Ok(None),
            Err(_) => Err("world catalogue unavailable"),
        },
        None => stored.get().await,
    };
    match stored {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err(ApiError::not_found(format!(
                "No place found with id {}",
                access.place_id
            )));
        }
        Err(error) => {
            tracing::error!(
                error,
                "place lookup failed during the presenter authz check"
            );
            return Err(service_unavailable(
                "Place verification is temporarily unavailable",
            ));
        }
    }
    // The signed scene is the room's: its explicit admins are granted on
    // its place key (`/scene-admin`), not on the credential's place.
    match scene::scene_manager_decision(state, &bound.place, &sf.signer).await {
        PermissionDecision::Allowed => Ok(room),
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
            Err(forbidden(NOT_PRESENTER_ADMIN_MSG))
        }
        PermissionDecision::Refused(why) => Err(why.into()),
    }
}

#[derive(Debug, Serialize)]
pub struct Presenters {
    pub presenters: Vec<String>,
}

pub async fn list_presenters(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Presenters>, ApiError> {
    let sf = verify_signed_fetch(
        &headers,
        "get",
        &SignedPath::at(&uri, PRESENTERS_PATH),
        &[SCENE_SIGNER],
        state.eip1654(),
    )
    .await?;
    let room = presenter_room(&state, &sf).await?;
    // Room metadata is the source of truth; an unreadable room has none.
    let info = match state.room_service().get_room_info(&room).await {
        Ok(info) => info,
        Err(error) => {
            tracing::warn!(room, ?error, "cast presenters: room info unavailable");
            None
        }
    };
    let metadata = crate::shared::livekit::parse_room_metadata(
        info.as_ref()
            .and_then(|i| i.get("metadata"))
            .and_then(Value::as_str),
    );
    let presenters = match metadata.get(PRESENTERS_FIELD) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    };
    Ok(Json(Presenters { presenters }))
}

/// The presenter route's signed path: the identity segment as requested.
fn presenter_path(uri: &Uri) -> SignedPath {
    let segment = uri.path().rsplit('/').next().unwrap_or_default();
    SignedPath::at(uri, format!("{PRESENTERS_PATH}/{segment}"))
}

fn checked_identity(identity: &str) -> Result<(), ApiError> {
    if is_valid_presenter_identity(identity) {
        Ok(())
    } else {
        Err(ApiError::bad_request(INVALID_PRESENTER_IDENTITY_MSG))
    }
}

pub async fn promote_presenter(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    Path(identity): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    // The signature first (upstream's route middleware), then the identity.
    let sf = verify_signed_fetch(
        &headers,
        "put",
        &presenter_path(&uri),
        &[SCENE_SIGNER],
        state.eip1654(),
    )
    .await?;
    checked_identity(&identity)?;
    let room = presenter_room(&state, &sf).await?;
    add_presenter(&state, &room, &identity).await?;
    tracing::info!(room, "cast presenter promoted");
    Ok(Json(
        json!({ "message": "Participant promoted to presenter" }),
    ))
}

pub async fn demote_presenter(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    Path(identity): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let sf = verify_signed_fetch(
        &headers,
        "delete",
        &presenter_path(&uri),
        &[SCENE_SIGNER],
        state.eip1654(),
    )
    .await?;
    checked_identity(&identity)?;
    let room = presenter_room(&state, &sf).await?;
    state
        .room_service()
        .remove_from_room_metadata_array(&room, PRESENTERS_FIELD, &identity)
        .await
        .map_err(|e| room_service_failure(&room, e))?;
    tracing::info!(room, "cast presenter demoted");
    Ok(Json(
        json!({ "message": "Participant demoted from presenter" }),
    ))
}

#[cfg(test)]
mod publisher_tests {
    use super::*;

    /// A streamer is read from the right, whatever its place holds; a bot
    /// only in its own room.
    #[test]
    fn publishers_classify_by_credential_whatever_their_place() {
        let (access, uuid) = (Uuid::new_v4(), Uuid::new_v4());
        let room = "scene:LocalPreview:bafkreiscene";
        for place in [
            "8f9e1529-cb0d-5b49-927e-b6fea90116c3",
            "scene:LocalPreview:bafkreiscene",
        ] {
            assert_eq!(
                classify_publisher(&format!("stream:{place}:{access}:{uuid}"), room),
                Publisher::Tagged(access),
                "{place}"
            );
            assert_eq!(
                classify_publisher(&format!("stream:{place}:{uuid}"), room),
                Publisher::Untagged,
                "{place}"
            );
        }
        assert_eq!(
            classify_publisher(&format!("presentation-bot:{room}:{access}:{uuid}"), room),
            Publisher::Tagged(access)
        );
        for other in [
            format!("presentation-bot:another-room:{access}:{uuid}"),
            format!("stream:{access}:not-a-uuid"),
            format!("stream::{uuid}"),
            "0x0000000000000000000000000000000000000001".to_owned(),
        ] {
            assert_eq!(
                classify_publisher(&other, room),
                Publisher::Other,
                "{other}"
            );
        }
    }
}
