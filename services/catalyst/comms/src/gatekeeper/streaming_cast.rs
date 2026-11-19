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
//!   access sweeps reach a watcher already in the room.
//! - `GET /presenters`, `PUT|DELETE /presenters/{identity}` (signed as
//!   scene): owner or admin of the place the room's stream credential is
//!   stored for. As upstream, a demotion does not stick for a scene admin:
//!   the next join of that admin adds them back.
//!
//! Publisher identities name the credential that minted them
//! (`stream:<place>:<access id>:<uuid>`, `presentation-bot:<room>:<access
//! id>:<uuid>`; upstream has `stream:<place>:<uuid>`). Revoking or replacing a
//! credential removes its publishers and their presenter entries, and the
//! `participant_joined` webhook removes one that connects later with a token
//! still inside its five-minute lifetime.
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
use uuid::Uuid;

use super::super::admission::scene::{self, PlaceLookup};
use super::super::auth::{
    SCENE_SIGNER, SignedFetch, device_identifier, verify_signed_fetch, verify_viewer_signed_fetch,
};
use super::super::errors::{ApiError, forbidden, service_unavailable, unauthorized};
use super::super::livekit::{AccessToken, TRACK_SOURCE_MICROPHONE, VideoGrants};
use super::super::{CommsState, SharedState};
use super::{
    Context, FOUR_DAYS_MS, NOT_AUTHORIZED_TO_STREAM_MSG, active_access, issue_access,
    refuse_if_platform_banned, resolve_place_id,
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
const PLACE_NOT_FOUND_MSG: &str = "place not found for this scene";
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
        "SELECT id, place_id, room_id, created_at, expiration_time FROM social.scene_stream_access \
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
    sqlx::query_as(
        "SELECT id, place_id, room_id, created_at, expiration_time FROM social.scene_stream_access \
         WHERE room_id = $1 AND active \
         AND COALESCE(expiration_time, created_at + interval '4 days') > timezone('UTC', now()) \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(room)
    .fetch_optional(pool)
    .await
}

/// A key's credential for the publishing tokens: unknown 401, expired 401,
/// without a room 401 (upstream's `validateStreamerToken` and
/// `generatePresentationBotToken`). Returns the place, the room and the
/// credential's id, which the publisher identities carry.
async fn publishing_access(pool: &PgPool, key: &str) -> Result<(String, String, Uuid), ApiError> {
    let access = access_by_key(pool, key)
        .await?
        .ok_or_else(|| unauthorized(INVALID_STREAMING_KEY_MSG))?;
    if now_ms() >= expires_ms(access.created_at, access.expiration_time) {
        return Err(unauthorized(EXPIRED_STREAMING_KEY_MSG));
    }
    let room = access
        .room_id
        .ok_or_else(|| unauthorized(INVALID_STREAMING_KEY_MSG))?;
    Ok((access.place_id, room, access.id))
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

/// Classifies `identity` against the streamer prefix of `place_id` and the
/// bot prefix of `room`.
fn classify_publisher(identity: &str, place_id: &str, room: &str) -> Publisher {
    let rest = identity
        .strip_prefix("stream:")
        .and_then(|r| r.strip_prefix(place_id))
        .or_else(|| {
            identity
                .strip_prefix("presentation-bot:")
                .and_then(|r| r.strip_prefix(room))
        })
        .and_then(|r| r.strip_prefix(':'));
    let Some(rest) = rest else {
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

/// A watch location's place: a name ending in `.eth` is a world, anything
/// else a Genesis parcel, resolved as the stream link keyed its credential.
async fn watch_place(pool: &PgPool, location: &str) -> Result<(String, CastPlace), ApiError> {
    let id = if location.to_lowercase().ends_with(".eth") {
        let not_found =
            || ApiError::not_found(format!("No world scene place found for world {location}"));
        let world = WorldName::parse(location).map_err(|_| not_found())?;
        resolve_place_id(pool, Some(&world), "", false)
            .await?
            .ok_or_else(not_found)?
    } else {
        let not_found = || ApiError::not_found(format!("No place found with parcel {location}"));
        // Upstream matches the place's `positions` verbatim.
        match Parcel::parse(location) {
            Ok(parcel) if parcel.canonical() == location => {
                resolve_place_id(pool, None, location, false)
                    .await?
                    .ok_or_else(not_found)?
            }
            _ => return Err(not_found()),
        }
    };
    let place = load_place(pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("No place found with id {id}")))?;
    Ok((id, place))
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
                Err(ResolveError::Db(error)) => Err(AuthorityNotEstablished::store_unavailable(
                    "worlds.world_scenes",
                    &error,
                )
                .into()),
                Err(_) => Ok(None),
            }
        }
        _ => Ok(None),
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

/// Upstream's `addPresenter` for a new streamer id, in one read and one write
/// of the room's metadata: streamer ids of the place that are not in the room
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
    refuse_if_platform_banned(&state, &sf.signer, device_id.as_deref()).await?;
    if state.deny_list.is_denied(sf.signer.as_str()).await {
        return Err(forbidden(NOT_AUTHORIZED_TO_STREAM_MSG));
    }
    // The room and the authorizing place both come from the one bound scene.
    let bound = Context::resolve(&state, &sf.metadata).await?;
    let world = bound.world_scene.as_ref().map(|(world, _)| world);
    let base = bound.footprint.base_position.clone().unwrap_or_default();
    let place_id = resolve_place_id(&state.pool, world, &base, true)
        .await?
        .ok_or_else(|| ApiError::not_found(PLACE_NOT_FOUND_MSG))?;
    // Explicit admins are stored under the bound scene's entity id
    // (`/scene-admin`); the credential stays under the catalogue place, and
    // a world's one credential takes authority over the whole world.
    let authority = super::key_authority(&state, &place_id, world, bound.place);
    match scene::scene_manager_decision(&state, &authority, &sf.signer).await {
        PermissionDecision::Allowed => {}
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
            tracing::warn!(signer = %sf.signer, place_id, "stream link refused: not a scene admin");
            return Err(forbidden(NOT_STREAM_LINK_ADMIN_MSG));
        }
        PermissionDecision::Refused(why) => return Err(why.into()),
    }
    let access = issue_access(&state, &state.ingress(), &place_id, &bound.room, &sf.signer).await?;
    let row = load_place(&state.pool, &place_id).await?;
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
    let place = load_place(&state.pool, &access.place_id)
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
    let (place_id, room, access_id) = publishing_access(&state.pool, key).await?;
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
    refuse_if_platform_banned(&state, watcher, device_id.as_deref()).await?;
    if state.deny_list.is_denied(watcher.as_str()).await {
        return Err(unauthorized(super::super::voice::DENY_LISTED_MSG));
    }
    let (place_id, place) = watch_place(&state.pool, location).await?;
    let access = active_access(&state.pool, &place_id)
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
    let ban_key = scene_room
        .ban_key()
        .ok_or_else(|| no_active_stream(location))?;
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
    let (_place_id, room, access_id) = publishing_access(&state.pool, key).await?;
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
    place_id: &str,
    room: &str,
    keep: Option<Uuid>,
) -> Result<(), ApiError> {
    if !state.livekit.configured {
        return Ok(());
    }
    let rooms = state.room_service();
    let revoked = |identity: &str| match classify_publisher(identity, place_id, room) {
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
/// old session may have removed.
pub async fn on_publisher_joined(
    state: &CommsState,
    room: &str,
    identity: &str,
) -> Result<(), ApiError> {
    if refuse_revoked_publisher(state, room, identity).await? || !identity.starts_with("stream:") {
        return Ok(());
    }
    if !state.livekit.configured {
        return Ok(());
    }
    state
        .room_service()
        .append_to_room_metadata_array(room, PRESENTERS_FIELD, identity)
        .await
        .map_err(|e| ApiError::internal(format!("cast presenters: {e}")))
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
    let active = access_by_room(&state.pool, room).await?;
    let allowed = match (tagged_access(identity), &active) {
        (Some(access), Some(active)) => access == active.id,
        (Some(_), None) => false,
        // Untagged: kept while the room still streams, as before tagging.
        (None, active) => active.is_some(),
    };
    if allowed {
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
    // The room's own scene: explicit admins are stored under its entity id.
    let place = match room_scene(state, room).await? {
        Some(resolved) => super::scene_authority(state, &resolved, &access.place_id).await?,
        None => None,
    }
    .unwrap_or_else(|| PlaceLookup::new(state, &access.place_id));
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
/// else 403.
async fn presenter_room(state: &CommsState, sf: &SignedFetch) -> Result<String, ApiError> {
    require_scene_metadata(sf, "sceneId is required in authMetadata")?;
    require_livekit(state)?;
    let bound = Context::resolve(state, &sf.metadata).await?;
    let room = bound.room;
    let access = access_by_room(&state.pool, &room)
        .await?
        .ok_or_else(|| no_active_stream(&room))?;
    let stored = PlaceLookup::new(state, &access.place_id);
    match stored.get().await {
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
    // The signed scene is the room's: its explicit admins are stored under
    // its entity id (`/scene-admin`), not under the credential's place.
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
