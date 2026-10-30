//! Scene and world identity, and who may moderate a scene.
//!
//! The old gatekeeper resolved a world's scene over HTTP (`/world/{name}/about`
//! of the worlds service), a parcel's scene over HTTP (`/content/entities/
//! active`), a place over the places database and ownership over the squid
//! index. On v2 the first three are the ONE database: a world's active scene
//! is the newest `worlds.world_scenes` row (what `/world/{name}/about` itself
//! serves as `scenesUrn[0]`), a parcel's scene is `content.active_pointers`,
//! a place is `explore.place`. The client and the authoritative host
//! therefore name the same room from the same active deployment
//! (DATABASE.md, "Presence is not proof of permission"). Ownership is read
//! from the marketplace subgraph through the marketplace module (ENS names
//! for a world, parcel and estate owners by coordinates for a Genesis
//! place, the old crate's squid `nft` lookups) through the worlds
//! permission core ([`crate::interfaces::AccessPolicy`]), which also answers world
//! access and the protected wallets of a world place where the old
//! gatekeeper fetched the worlds service's `/permissions` document; the
//! LAND operators still come from a configured upstream through the guarded
//! client, and an unreadable surface never clears a protection.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;
use sqlx::Row;

use crate::access::{Access, AccessType, LandOperators, SceneRef, WorldStanding};
use crate::shared::cache::TtlMap;
use crate::world_identity::{Parcel, ParcelSet, SceneId, WorldName, is_world_realm_name};
use crate::{Address, AuthorityNotEstablished, PermissionDecision};

use crate::gatekeeper::CommsState;
use crate::gatekeeper::errors::{ApiError, service_unavailable};
use crate::gatekeeper::livekit::{BANNED_ADDRESSES_FIELD, KICK_CONCURRENCY, SCENE_ADMINS_FIELD};
use crate::rooms::{ResolveError, ResolvedRoom, RoomRequest, gatekeeper_scene_room};

pub const WORLD_PERMISSIONS_UNAVAILABLE_MSG: &str = "the world permission store is unavailable, so this ban target cannot be checked for protection";
pub const WORLD_ACCESS_UNAVAILABLE_MSG: &str =
    "the world permission store is unavailable, so world access cannot be decided";
pub const LAND_OPERATORS_UNAVAILABLE_MSG: &str = "the land permission service is unavailable, so this ban target cannot be checked for protection";
pub const PLACE_LOOKUP_UNAVAILABLE_MSG: &str =
    "the place catalog is unavailable, so this request cannot be bound to its room";
const PLACE_MISMATCH_MSG: &str =
    "place_id does not match the realm and parcel this request is signed for";
pub const NOT_OWNER_OR_ADMIN_MSG: &str = "signer is not an owner or admin of this scene";

const LEASE_AUTHORIZATIONS_TTL: Duration = Duration::from_secs(300);
const NAMES_TTL: Duration = Duration::from_secs(300);
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn meta_str(meta: &Value, key: &str) -> Option<String> {
    meta.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn realm_name_from_metadata(metadata: &Value) -> Option<String> {
    meta_str(metadata, "realmName").or_else(|| {
        metadata
            .get("realm")
            .and_then(|r| meta_str(r, "serverName"))
    })
}

/// The place a scene-signed request stands on: the world name for a world
/// realm, else the scene id.
pub fn place_from_metadata(meta: &Value) -> Option<String> {
    if let Some(realm) = realm_name_from_metadata(meta)
        && is_world_realm_name(&realm)
    {
        return Some(realm);
    }
    meta_str(meta, "sceneId")
}

/// The key the scene-bans listings use: a world realm keys on its scene hash
/// when the metadata carries one, else on the world name (resolved below).
pub(crate) fn listing_key_candidate(meta: &Value) -> Option<String> {
    let realm_name = realm_name_from_metadata(meta);
    let scene_id = meta_str(meta, "sceneId");
    match realm_name {
        Some(realm) if is_world_realm_name(&realm) => match scene_id {
            Some(id) if !is_world_realm_name(&id) => Some(id),
            _ => Some(realm),
        },
        _ => scene_id,
    }
}

/// Keep raw lookup keys: parsing must never turn a missing raw key into a hit.
fn raw_world(world: &str) -> Option<WorldName> {
    WorldName::parse(world)
        .ok()
        .filter(|parsed| parsed.key() == world.to_lowercase())
}

pub(crate) async fn active_scene_id(
    state: &CommsState,
    world: &str,
) -> Result<Option<String>, ApiError> {
    let Some(world) = raw_world(world) else {
        return Ok(None);
    };
    state
        .scenes
        .world_active_scene(&world)
        .await
        .map(|dep| dep.map(|dep| dep.scene_id.to_string()))
        .map_err(|error| ApiError::internal(error.to_string()))
}

pub(crate) async fn participant_room(
    state: &CommsState,
    realm: &str,
    pointer: &str,
) -> Result<Option<ResolvedRoom>, ApiError> {
    let Ok(parcel) = Parcel::parse(pointer) else {
        return Ok(None);
    };
    if parcel.canonical() != pointer {
        return Ok(None);
    }
    let realm = if is_world_realm_name(realm) {
        let Some(world) = raw_world(realm) else {
            return Ok(None);
        };
        crate::world_identity::RealmId::World(world)
    } else {
        crate::world_identity::RealmId::Shared(realm.to_string())
    };
    match state
        .scenes
        .resolve(crate::rooms::RoomRequest::Parcel {
            realm: &realm,
            parcel,
        })
        .await
    {
        Ok(room) => Ok(Some(room)),
        Err(crate::rooms::ResolveError::NoSceneAtParcel { .. }) => Ok(None),
        Err(error) => Err(ApiError::internal(error.to_string())),
    }
}

/// A world realm with a world-shaped scene id resolves to the world's active
/// scene; an explicit id under a world realm must be a scene deployed in
/// that world (the ONE room resolver's rows; the worlds service answered 404
/// for any other, and a room is minted for active deployments only, never
/// for a replaced version or an invented id); a shared realm's id is taken
/// as given (the old gatekeeper's contract for `main`/preview realms).
pub async fn resolve_world_scene_id(
    state: &CommsState,
    realm_name: &str,
    scene_id: &str,
) -> Result<String, ApiError> {
    resolve_adapter_room(state, realm_name, scene_id)
        .await?
        .scene_id
        .map(SceneId::into_string)
        .ok_or_else(|| ApiError::internal("scene resolution returned a lobby"))
}

/// Resolve once, retaining the room and the deployment's original scene key.
/// Signed keys must already be well-formed: parsing must not trim a request
/// into a different deployment. Shared/preview realms retain their signed
/// room bytes and require no catalogue/database lookup.
pub(crate) async fn resolve_adapter_room(
    state: &CommsState,
    realm_name: &str,
    scene_id: &str,
) -> Result<ResolvedRoom, ApiError> {
    let name = WorldName::parse(realm_name)
        .map_err(|e| ApiError::bad_request(format!("Invalid realm name: {e}")))?;
    if name.display() != realm_name {
        return Err(ApiError::bad_request("Invalid realm name"));
    }
    let scene = SceneId::parse(scene_id)
        .map_err(|e| ApiError::bad_request(format!("Invalid scene id: {e}")))?;
    if scene.as_str() != scene_id {
        return Err(ApiError::bad_request("Invalid scene id"));
    }
    if !is_world_realm_name(realm_name) {
        if realm_name.contains(':') || scene_id.contains(':') {
            return Err(ApiError::bad_request("Invalid shared realm or scene id"));
        }
        return Ok(ResolvedRoom {
            realm: crate::world_identity::RealmId::Shared(realm_name.to_string()),
            scene_id: Some(scene),
            deployment: None,
            room: gatekeeper_scene_room(realm_name, scene_id),
        });
    }
    let realm = crate::world_identity::RealmId::World(name);
    let request = if scene.is_world_shaped() {
        RoomRequest::Adapter {
            realm: &realm,
            scene: &scene,
        }
    } else {
        RoomRequest::WorldScene {
            world: realm.world().unwrap(),
            scene: &scene,
        }
    };
    state
        .scenes
        .resolve(request)
        .await
        .map_err(|error| match error {
            ResolveError::SceneNotInWorld { .. } => ApiError::not_found(error.to_string()),
            ResolveError::WorldHasNoScene(_) => {
                ApiError::bad_request(format!("Failed to resolve scene ID for world {realm_name}"))
            }
            _ => ApiError::internal(error.to_string()),
        })
}

/// The scene-bans listing key: an explicit `place_id`, else the metadata's,
/// with a world name resolved to its active scene.
pub async fn resolve_listing_place_id(
    state: &CommsState,
    explicit: Option<String>,
    meta: &Value,
) -> Result<String, ApiError> {
    let candidate = explicit
        .filter(|s| !s.is_empty())
        .or_else(|| listing_key_candidate(meta))
        .ok_or_else(|| ApiError::bad_request("missing place_id query"))?;
    if !is_world_realm_name(&candidate) {
        return Ok(candidate);
    }
    match active_scene_id(state, &candidate).await? {
        Some(id) => Ok(id),
        None => Err(ApiError::bad_request(format!(
            "Failed to resolve scene ID for world {candidate}"
        ))),
    }
}

pub struct PlaceInfo {
    pub world: bool,
    pub world_name: Option<String>,
    pub positions: Vec<String>,
    pub base_position: Option<String>,
}

/// `explore.place` (the mirrored catalogue) then `explore.place_world_local`
/// (worlds published here); `Ok(None)` is "no such place".
pub async fn try_load_place_info(
    pool: &PgPool,
    place_id: &str,
) -> Result<Option<PlaceInfo>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT world, world_name, raw->'positions' AS positions, base_position FROM explore.place WHERE id = $1 \
         UNION ALL \
         SELECT world, world_name, raw->'positions' AS positions, base_position FROM explore.place_world_local WHERE id = $1 \
         LIMIT 1",
    )
    .bind(place_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(|row| {
        let world: bool = row.try_get("world").unwrap_or(false);
        let world_name: Option<String> = row.try_get("world_name").ok().flatten();
        let base_position: Option<String> = row.try_get("base_position").ok();
        let mut positions: Vec<String> = Vec::new();
        if let Ok(Some(Value::Array(arr))) = row.try_get::<Option<Value>, _>("positions") {
            positions.extend(arr.iter().filter_map(|p| p.as_str().map(str::to_string)));
        }
        if positions.is_empty()
            && let Some(bp) = base_position.as_deref()
        {
            positions.push(bp.to_string());
        }
        PlaceInfo {
            world,
            world_name,
            positions,
            base_position,
        }
    }))
}

/// One place row shared by every step of a request (authz, target
/// protection, room binding); read lazily.
pub struct PlaceLookup<'a> {
    state: &'a CommsState,
    place_id: String,
    cell: tokio::sync::OnceCell<Result<Option<PlaceInfo>, String>>,
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    pub(crate) fn cached_place<'a>(
        state: &'a CommsState,
        id: &str,
        info: PlaceInfo,
    ) -> PlaceLookup<'a> {
        PlaceLookup {
            state,
            place_id: id.to_owned(),
            cell: tokio::sync::OnceCell::new_with(Some(Ok(Some(info)))),
        }
    }
}

impl<'a> PlaceLookup<'a> {
    /// A place already bound to a current deployment by the ONE resolver.
    /// Moderation requests share this footprint for policy and room effects.
    pub(crate) fn resolved(state: &'a CommsState, place_id: &str, info: PlaceInfo) -> Self {
        Self {
            state,
            place_id: place_id.to_owned(),
            cell: tokio::sync::OnceCell::new_with(Some(Ok(Some(info)))),
        }
    }

    pub fn new(state: &'a CommsState, place_id: &str) -> Self {
        Self {
            state,
            place_id: place_id.to_string(),
            cell: tokio::sync::OnceCell::new(),
        }
    }

    pub fn place_id(&self) -> &str {
        &self.place_id
    }

    /// `Err` carries the catalogue error text; callers map it to their status.
    pub async fn get(&self) -> Result<Option<&PlaceInfo>, &str> {
        self.cell
            .get_or_init(|| async {
                try_load_place_info(&self.state.pool, &self.place_id)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await
            .as_ref()
            .map(Option::as_ref)
            .map_err(String::as_str)
    }
}

/// The world of a realm name as the permission core keys it.
fn world_of(realm_or_name: &str) -> Option<WorldName> {
    WorldName::parse(realm_or_name).ok()
}

fn parcel_set(positions: &[String]) -> ParcelSet {
    positions
        .iter()
        .filter_map(|p| Parcel::parse(p).ok())
        .collect()
}

/// Whether `identity` may enter `world_name` (the scene `scene_id` of it,
/// when known): `Policy::can_access`, the local call that replaced the
/// worlds service's `/permissions` document. An unknown, operator-disabled
/// or deny-listed world admits nobody (the old `isWorldValid`). The owner,
/// an unrestricted world, an allow-listed wallet or a matching shared secret
/// (the `secret` field of the signed metadata, the scene host's contract)
/// are admitted; a ban, an allow-list miss, an nft-ownership world and a
/// wrong or missing secret are not. A shared-secret world's attempts spend
/// the world mints' budget per world and `caller` (429 past it). A store
/// that cannot answer is a 503, never a denial.
pub async fn has_world_access_permission(
    state: &CommsState,
    identity: &Address,
    caller: &str,
    world_name: &str,
    scene: Option<&crate::access::SceneBanKey>,
    secret: Option<&str>,
    device_id: Option<&str>,
) -> Result<bool, ApiError> {
    let Some(world) = world_of(world_name) else {
        return Ok(false);
    };
    let unavailable = |why: AuthorityNotEstablished| {
        tracing::error!(error = %why, world = %world, "world access could not be decided");
        service_unavailable(WORLD_ACCESS_UNAVAILABLE_MSG)
    };
    let shared_secret = match state.policy.world_standing(&world).await {
        Ok(WorldStanding::Open(access_type)) => access_type == AccessType::SharedSecret,
        Ok(WorldStanding::Unknown | WorldStanding::Disabled | WorldStanding::DenyListed) => {
            return Ok(false);
        }
        Err(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => return Ok(false),
        Err(why) => return Err(unavailable(why)),
    };
    let now = chrono::Utc::now().naive_utc();
    let throttle = crate::gatekeeper::admission::world_mint::SecretThrottle {
        pool: &state.pool,
        max_attempts: state.world_mint.secret_max_attempts,
        world: world.key(),
        subject: caller,
    };
    if shared_secret && throttle.reserve(now).await {
        return Err(ApiError::http(
            429,
            "Too many shared-secret attempts. Try again later.",
        ));
    }
    let access = state
        .policy
        .can_access(&world, identity, secret, scene, device_id, now)
        .await;
    if shared_secret {
        throttle
            .settle(matches!(access, Ok(Access::Allowed(_))))
            .await;
    }
    match access {
        Ok(Access::Allowed(_)) => Ok(true),
        Ok(Access::Banned(_) | Access::NotAllowed | Access::InvalidSecret) => Ok(false),
        Err(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => Ok(false),
        Err(why) => Err(unavailable(why)),
    }
}

#[derive(Debug, Deserialize)]
struct LeaseAuthorization {
    #[serde(default)]
    addresses: Vec<String>,
    #[serde(default)]
    plots: Vec<String>,
}

/// The upstream caches: the lease authorizations and the ENS names.
pub struct SceneCaches {
    lease_holders: TtlMap<String, Arc<Vec<LeaseAuthorization>>>,
    names: TtlMap<String, String>,
}

impl Default for SceneCaches {
    fn default() -> Self {
        Self {
            lease_holders: TtlMap::new(LEASE_AUTHORIZATIONS_TTL),
            names: TtlMap::bounded(NAMES_TTL, 50_000),
        }
    }
}

fn parse_xy(s: &str) -> Option<(i32, i32)> {
    let mut it = s.splitn(2, ',');
    Some((
        it.next()?.trim().parse().ok()?,
        it.next()?.trim().parse().ok()?,
    ))
}

/// The LAND manager document of a Genesis parcel through the permission
/// core (`Policy::land_document`: the `land_authz` index once its fold is
/// published, else `COMMS_LAND_OPERATORS_URL`). Upstream's `getLandOperators`
/// re-throws on a transport error and on any non-2xx, so a source fault
/// reaches the caller as a rejection and never as "this parcel has no
/// operators". Without any source v2 has no LAND evidence at all: that is
/// the old "places pool unset" degraded mode (no extra address is protected
/// on Genesis places), logged, not a 503.
async fn try_fetch_land_operators(
    state: &CommsState,
    parcel: &str,
) -> Result<Option<Arc<Option<LandOperators>>>, ApiError> {
    let Some((x, y)) = parse_xy(parcel) else {
        return Ok(None);
    };
    match state.policy.land_document(Parcel { x, y }).await {
        Ok(Some(doc)) => Ok(Some(doc)),
        Ok(None) => {
            tracing::debug!(
                parcel,
                "no LAND operator source (land-authz index not ready, COMMS_LAND_OPERATORS_URL unset); no LAND operator is protected"
            );
            Ok(None)
        }
        Err(e) => {
            tracing::warn!(error = %e, parcel, "LAND operators could not be read");
            Err(service_unavailable(LAND_OPERATORS_UNAVAILABLE_MSG))
        }
    }
}

/// The addresses a place's other permission surfaces grant: the world's
/// owner and deployment/streaming wallets, or the parcel's LAND owner and
/// operators. A surface that cannot be read is an error, never an empty set
/// (an empty set here reads as "nobody is protected").
pub async fn try_get_extra_addresses(
    state: &CommsState,
    place: &PlaceInfo,
) -> Result<BTreeSet<String>, ApiError> {
    let mut extra: BTreeSet<String> = BTreeSet::new();
    if place.world {
        let Some(world) = place.world_name.as_deref().and_then(world_of) else {
            return Ok(extra);
        };
        // The known owner plus every deployment/streaming grantee whose grant
        // is world-wide or touches the place's parcels (the old
        // `/permissions/{perm}/parcels` listings plus the document's owner).
        let protected = state
            .policy
            .protected_addresses(&world, &parcel_set(&place.positions))
            .await
            .map_err(|why| {
                tracing::error!(error = %why, world = %world, "protected addresses could not be read");
                service_unavailable(WORLD_PERMISSIONS_UNAVAILABLE_MSG)
            })?;
        extra.extend(protected.into_iter().map(|a| a.as_str().to_string()));
    } else {
        let parcel = place
            .base_position
            .clone()
            .or_else(|| place.positions.first().cloned());
        if let Some(parcel) = parcel
            && let Some(doc) = try_fetch_land_operators(state, &parcel).await?
            && let Some(ops) = doc.as_ref()
        {
            extra.extend(ops.addresses());
        }
    }
    Ok(extra)
}

/// The listing's best-effort variant: an outage reads as no extra address.
pub async fn get_extra_addresses(state: &CommsState, place: &PlaceInfo) -> BTreeSet<String> {
    try_get_extra_addresses(state, place)
        .await
        .unwrap_or_default()
}

/// Holders of a linker-server lease over any of `parcels`, from the document
/// at `LEASE_AUTHORIZATIONS_URL` (the mirror posture's: Decentraland's
/// linker-server-authorizations list); no URL or an unreadable document
/// reads as no holder, as upstream.
pub async fn get_lease_holders_for_parcels(
    state: &CommsState,
    parcels: &[String],
) -> BTreeSet<String> {
    match try_get_lease_holders_for_parcels(state, parcels).await {
        Ok(holders) => holders,
        Err(error) => {
            tracing::debug!(error = %error.message, "lease authorizations unavailable; no lease holder is listed");
            BTreeSet::new()
        }
    }
}

/// Metadata refresh must preserve its previous roles when a configured reader
/// fails. Other listings retain the best-effort wrapper above.
pub(crate) async fn try_get_lease_holders_for_parcels(
    state: &CommsState,
    parcels: &[String],
) -> Result<BTreeSet<String>, ApiError> {
    let mut holders: BTreeSet<String> = BTreeSet::new();
    let Some(url) = state.settings.lease_authorizations_url.as_deref() else {
        return Ok(holders);
    };
    if parcels.is_empty() {
        return Ok(holders);
    }
    let fetched = state
        .caches
        .lease_holders
        .get_or_fetch("leases".to_string(), || async {
            let resp = state
                .http
                .get(url)
                .timeout(UPSTREAM_TIMEOUT)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("lease authorizations returned {}", resp.status()));
            }
            resp.json::<Vec<LeaseAuthorization>>()
                .await
                .map(Arc::new)
                .map_err(|e| e.to_string())
        })
        .await;
    let auths = match fetched {
        Ok(auths) => auths,
        Err(e) => {
            tracing::debug!(error = %e, "lease authorizations unavailable");
            return Err(service_unavailable(
                "Lease authorizations are temporarily unavailable",
            ));
        }
    };
    let parcel_set: BTreeSet<&str> = parcels.iter().map(String::as_str).collect();
    for auth in auths.iter() {
        if auth
            .plots
            .iter()
            .any(|plot| parcel_set.contains(plot.as_str()))
        {
            holders.extend(auth.addresses.iter().map(|a| a.to_lowercase()));
        }
    }
    Ok(holders)
}

/// ENS display names for a bounded set of wallets through the marketplace
/// names reader (the old crate read the squid `ens` table). Unconfigured or
/// failing reads answer `""`, as before.
pub async fn names_for(
    state: &CommsState,
    addresses: &[String],
) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let Some(market) = state.marketplace.as_ref() else {
        for addr in addresses {
            out.insert(addr.to_lowercase(), String::new());
        }
        return out;
    };
    for addr in addresses {
        let addr = addr.to_lowercase();
        let name = state
            .caches
            .names
            .get_or_fetch(addr.clone(), || async {
                match crate::chain_index::graph::owned_names(market, &addr).await {
                    Ok(names) => Ok::<String, ()>(names.first().map(|n| n.name.clone()).unwrap_or_default()),
                    Err(e) => {
                        tracing::warn!(error = %e, "names resolve failed; falling back to an empty name");
                        Ok(String::new())
                    }
                }
            })
            .await
            .unwrap_or_default();
        out.insert(addr, name);
    }
    out
}

/// An explicit active `scene_admins` row, else ownership of the place: the
/// world's NAME (personal rule, the stored owner, then the name authority)
/// for a world, the LAND (a parcel directly or through its estate) for a
/// Genesis place, all through `Policy::is_admin`. Without a marketplace
/// reader a non-admin caller is refused (the old crate's "only explicit
/// scene admins can moderate" degrade); a reader that cannot be asked is
/// 503, never "not an owner".
pub async fn scene_owner_or_admin_decision(
    state: &CommsState,
    place: &PlaceLookup<'_>,
    signer: &Address,
) -> PermissionDecision {
    let place_id = place.place_id();
    let Ok(scene) = SceneId::parse(place_id) else {
        return PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG);
    };
    let place_row = match place.get().await {
        Ok(row) => row,
        Err(error) => {
            tracing::error!(
                error,
                place_id,
                "place lookup failed during the scene authz check"
            );
            return PermissionDecision::Refused(
                AuthorityNotEstablished::UndeterminedStoreUnavailable {
                    store: "explore.place",
                    reason_for_operators: error.to_string(),
                },
            );
        }
    };
    let empty = ParcelSet::new();
    let (world, parcels) = match place_row {
        Some(row) if row.world => (row.world_name.as_deref().and_then(world_of), empty),
        Some(row) => (None, parcel_set(&row.positions)),
        None => (None, empty),
    };
    let scene_ref = match world.as_ref() {
        Some(world) => SceneRef::World {
            world,
            scene: &scene,
        },
        None => SceneRef::Genesis {
            scene: &scene,
            parcels: &parcels,
        },
    };
    match state.policy.is_admin(&scene_ref, signer).await {
        PermissionDecision::Allowed => PermissionDecision::Allowed,
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
            PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG)
        }
        undetermined => undetermined,
    }
}

/// Owners, admins, extra addresses and lease holders cannot be banned. A
/// protection surface that cannot be read is answered apart from a genuine
/// "not protected": 503, never "nobody is protected".
pub async fn ensure_target_not_protected(
    state: &CommsState,
    place: &PlaceLookup<'_>,
    target: &Address,
) -> Result<(), ApiError> {
    match scene_owner_or_admin_decision(state, place, target).await {
        PermissionDecision::Allowed => {
            return Err(ApiError::bad_request("Cannot ban this address"));
        }
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {}
        PermissionDecision::Refused(why) => return Err(why.into()),
    }
    let place_id = place.place_id().to_string();
    let row = place.get().await.map_err(|error| {
        tracing::error!(error, %place_id, "places lookup failed while checking a ban target for protection");
        service_unavailable(PLACE_LOOKUP_UNAVAILABLE_MSG)
    })?;
    if let Some(row) = row {
        let (extra, leases) = tokio::join!(try_get_extra_addresses(state, row), async {
            if row.world {
                BTreeSet::new()
            } else {
                get_lease_holders_for_parcels(state, &row.positions).await
            }
        });
        let mut protected = extra?;
        protected.extend(leases);
        if protected.contains(target.as_str()) {
            return Err(ApiError::bad_request("Cannot ban this address"));
        }
    }
    Ok(())
}

/// The request context upstream derives a room name from
/// (`livekit.getRoomName(realmName, { isWorld, sceneId })`), beside the
/// `place_id` the endpoints are keyed on.
pub struct RoomContext {
    pub place_id: String,
    pub realm_name: Option<String>,
    pub scene_id: Option<String>,
    pub parcel: Option<String>,
}

impl RoomContext {
    pub fn from_metadata(metadata: &Value, place_id: &str) -> Self {
        Self {
            place_id: place_id.to_string(),
            realm_name: realm_name_from_metadata(metadata),
            scene_id: meta_str(metadata, "sceneId"),
            parcel: meta_str(metadata, "parcel"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum RoomPlan {
    Skip,
    Scene(String),
    World {
        world: String,
        scene_id: Option<String>,
    },
    FromPlace,
}

fn plan(ctx: &RoomContext) -> RoomPlan {
    let Some(realm) = ctx.realm_name.as_deref() else {
        return RoomPlan::FromPlace;
    };
    if is_world_realm_name(realm) {
        return RoomPlan::World {
            world: realm.to_string(),
            scene_id: ctx.scene_id.clone(),
        };
    }
    match ctx.scene_id.as_deref() {
        Some(scene_id) => RoomPlan::Scene(gatekeeper_scene_room(realm, scene_id)),
        None => RoomPlan::Skip,
    }
}

/// The world branch keys on the `.eth` reading of the realm name, where
/// upstream decides `isWorld` from the signed realm's hostname.
fn context_matches_place(ctx: &RoomContext, place: &PlaceInfo) -> bool {
    match ctx.realm_name.as_deref() {
        None => true,
        Some(realm) if is_world_realm_name(realm) => {
            place.world
                && place
                    .world_name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(realm))
        }
        Some(_) => {
            !place.world
                && ctx.parcel.as_deref().is_some_and(|parcel| {
                    place.positions.iter().any(|p| p == parcel)
                        || place.base_position.as_deref() == Some(parcel)
                })
        }
    }
}

/// Upstream reads the place out of the same signed metadata it names the
/// room from; our endpoints take `place_id` from the request, so the two are
/// tied back together here, else an admin of one place could aim the LiveKit
/// side effects at another scene's room.
async fn ensure_place_matches_context(
    ctx: &RoomContext,
    place: &PlaceLookup<'_>,
) -> Result<(), ApiError> {
    let row = place.get().await.map_err(|error| {
        tracing::error!(error, place_id = %ctx.place_id, "places lookup failed while binding a mutation to its room");
        service_unavailable(PLACE_LOOKUP_UNAVAILABLE_MSG)
    })?;
    match row {
        Some(row) if context_matches_place(ctx, row) => Ok(()),
        _ => Err(ApiError::bad_request(PLACE_MISMATCH_MSG)),
    }
}

async fn world_rooms(
    state: &CommsState,
    world: &str,
    scene_id: Option<&str>,
) -> Result<Vec<String>, ApiError> {
    let world_name = raw_world(world).ok_or_else(|| ApiError::bad_request("Invalid realm name"))?;
    let mut rooms = vec![crate::rooms::worlds_service_world_room(&world_name)];
    // A catalogue WORLD can use a legacy arbitrary name without `.eth`.
    // Its known world scope must never fall through to a Genesis room.
    let scene = match scene_id {
        Some(raw) if !is_world_realm_name(raw) => {
            let parsed = SceneId::parse(raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
            if parsed.as_str() != raw {
                return Err(ApiError::bad_request("Invalid scene id"));
            }
            Some(parsed)
        }
        _ => state
            .scenes
            .world_active_scene(&world_name)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
            .map(|dep| dep.scene_id),
    };
    if let Some(scene) = scene {
        let resolved = state
            .scenes
            .resolve(RoomRequest::WorldScene {
                world: &world_name,
                scene: &scene,
            })
            .await
            .map_err(|e| match e {
                ResolveError::SceneNotInWorld { .. } => ApiError::not_found(e.to_string()),
                _ => ApiError::internal(e.to_string()),
            })?;
        rooms.push(resolved.room);
    }
    Ok(rooms)
}

/// The LiveKit rooms a ban/admin mutation must reflect into, resolved before
/// the database write so a failure to name the room cannot leave the two out
/// of sync.
pub async fn resolve_rooms(
    state: &CommsState,
    ctx: &RoomContext,
    place: &PlaceLookup<'_>,
) -> Result<Vec<String>, ApiError> {
    if !state.livekit.configured {
        return Ok(Vec::new());
    }
    match plan(ctx) {
        RoomPlan::Skip => Ok(Vec::new()),
        RoomPlan::Scene(room) => {
            if !matches!(
                crate::rooms::parse_room(&room),
                Some(crate::rooms::ParsedRoom::Local(_))
            ) {
                return Err(ApiError::bad_request("Invalid shared realm or scene id"));
            }
            ensure_place_matches_context(ctx, place).await?;
            Ok(vec![room])
        }
        RoomPlan::World { world, scene_id } => {
            ensure_place_matches_context(ctx, place).await?;
            world_rooms(state, &world, scene_id.as_deref()).await
        }
        RoomPlan::FromPlace => Ok(match place.get().await.ok().flatten() {
            Some(row) if row.world => match row.world_name.as_deref() {
                Some(world) => world_rooms(state, world, None).await?,
                None => Vec::new(),
            },
            _ => Vec::new(),
        }),
    }
}

#[derive(Clone, Copy)]
enum Op {
    Append,
    Remove,
}

async fn mutate(state: &CommsState, rooms: &[String], field: &str, address: &Address, op: Op) {
    if !state.livekit.configured || rooms.is_empty() {
        return;
    }
    let client = state.room_service();
    futures::stream::iter(rooms)
        .for_each_concurrent(KICK_CONCURRENCY, |room| {
            let client = &client;
            async move {
                let result = match op {
                    Op::Append => client.append_to_room_metadata_array(room, field, address.as_str()).await,
                    Op::Remove => client.remove_from_room_metadata_array(room, field, address.as_str()).await,
                };
                if let Err(error) = result {
                    tracing::warn!(%error, room = %room, field, address = %address, "failed to sync scene room metadata (best-effort)");
                }
            }
        })
        .await;
}

pub async fn add_ban(state: &CommsState, rooms: &[String], address: &Address) {
    mutate(state, rooms, BANNED_ADDRESSES_FIELD, address, Op::Append).await;
}

pub async fn remove_ban(state: &CommsState, rooms: &[String], address: &Address) {
    mutate(state, rooms, BANNED_ADDRESSES_FIELD, address, Op::Remove).await;
}

pub async fn add_admin(state: &CommsState, rooms: &[String], address: &Address) {
    mutate(state, rooms, SCENE_ADMINS_FIELD, address, Op::Append).await;
}

pub async fn remove_admin(state: &CommsState, rooms: &[String], address: &Address) {
    mutate(state, rooms, SCENE_ADMINS_FIELD, address, Op::Remove).await;
}

/// Revocation takes effect in the running room: the banned wallet is removed
/// from every room the mutation named (best-effort, logged).
pub(crate) async fn kick(state: &CommsState, rooms: &[String], address: &Address) {
    if !state.livekit.configured || rooms.is_empty() {
        return;
    }
    let client = state.room_service();
    futures::stream::iter(rooms)
        .for_each_concurrent(KICK_CONCURRENCY, |room| {
            let client = &client;
            async move {
                if let Err(error) = client.remove_participant(room, address.as_str()).await {
                    tracing::warn!(%error, room = %room, address = %address, "failed to kick banned participant (best-effort)");
                }
            }
        })
        .await;
}

/// A complete inventory, or an explicit failure. No refusal is an empty scan.
pub(crate) async fn live_world_rooms(
    state: &CommsState,
    world: &WorldName,
) -> Result<Vec<String>, String> {
    use crate::rooms::RoomClassification;
    let rooms = state
        .room_service()
        .list_rooms()
        .await
        .map_err(|error| error.to_string())?;
    let mut selected = Vec::new();
    for room in rooms {
        // Reversible ownership survives normal redeployment. Admission, relay
        // and metadata still require an exact current catalogue tuple.
        if let Some((owner, _)) = crate::rooms::decode_world_scene_room(&room) {
            if owner == *world {
                selected.push(room);
            }
            continue;
        }
        match crate::rooms::classify_room_name(state.scenes.as_ref(), &room)
            .await
            .map_err(|error| error.to_string())?
        {
            RoomClassification::Canonical(resolved) if resolved.realm.world() == Some(world) => {
                selected.push(room)
            }
            RoomClassification::Canonical(_) | RoomClassification::Unrelated => {}
            RoomClassification::RelevantRefused { room, reason } => {
                return Err(format!("incomplete room inventory: {room:?}: {reason:?}"));
            }
        }
    }
    Ok(selected)
}

/// Evicts from `world`'s rooms every participant the access setting no
/// longer admits (the owner and deployers stay; a shared-secret participant
/// must mint again with the new secret, as the host requires). An undecided
/// participant stays.
pub(crate) async fn readmit_world_rooms(
    state: &CommsState,
    world: &WorldName,
) -> Result<(), String> {
    let privileged = state
        .policy
        .privileged_addresses(world)
        .await
        .map_err(|error| error.to_string())?;
    for room in live_world_rooms(state, world).await? {
        let client = state.room_service();
        for identity in client.list_participant_identities(&room).await {
            let Ok(address) = Address::parse(&identity) else {
                continue;
            };
            match crate::gatekeeper::revocation::evictable(
                state.policy.as_ref(),
                world,
                &privileged,
                &address,
                true,
            )
            .await
            {
                Ok(false) => {}
                Ok(true) => {
                    if let Err(error) = client.remove_participant(&room, &identity).await {
                        tracing::warn!(%error, room = %room, %address, "failed to evict a revoked participant (best-effort)");
                    }
                }
                // Fail open by design: a store fault never evicts, and this pass is not retried.
                Err(error) => {
                    tracing::warn!(%error, room = %room, %address, "access undecided; participant kept");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::test_support::{StubPolicy, StubScenes};
    use serde_json::json;

    #[tokio::test]
    async fn land_authority_fault_is_503_and_missing_evidence_stays_distinct() {
        let state_with = |policy: StubPolicy| {
            CommsState::new(
                sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
                crate::http::Outbound::for_tests(),
                crate::gatekeeper::Settings::for_tests(),
                None,
                None,
                Arc::new(policy),
                Arc::new(StubScenes::default()),
                Arc::new(crate::presence::Presence::new()),
                Arc::new(crate::interfaces::test_support::StubDirectory::default()),
            )
        };
        let no_source = state_with(StubPolicy::default());
        assert!(
            try_fetch_land_operators(&no_source, "1,2")
                .await
                .unwrap()
                .is_none()
        );
        let unknown = state_with(StubPolicy {
            land: Ok(Some(Arc::new(None))),
            ..Default::default()
        });
        assert!(
            try_fetch_land_operators(&unknown, "1,2")
                .await
                .unwrap()
                .unwrap()
                .is_none()
        );
        let unavailable = state_with(StubPolicy {
            land: Err(
                crate::shared::identity::AuthorityNotEstablished::UndeterminedStoreUnavailable {
                    store: "LAND operators reader",
                    reason_for_operators: "unavailable".into(),
                },
            ),
            ..Default::default()
        });
        let error = try_fetch_land_operators(&unavailable, "1,2")
            .await
            .unwrap_err();
        assert_eq!(error.code, 503);
        assert_eq!(error.message, LAND_OPERATORS_UNAVAILABLE_MSG);
    }

    #[test]
    fn parse_xy_handles_coords() {
        assert_eq!(parse_xy("-100,37"), Some((-100, 37)));
        assert_eq!(parse_xy(" 12 , -5 "), Some((12, -5)));
        assert_eq!(parse_xy("bad"), None);
        assert_eq!(parse_xy(""), None);
    }

    fn ctx(realm: Option<&str>, scene: Option<&str>) -> RoomContext {
        RoomContext {
            place_id: "place-1".into(),
            realm_name: realm.map(str::to_string),
            scene_id: scene.map(str::to_string),
            parcel: None,
        }
    }

    fn ctx_at(realm: Option<&str>, parcel: Option<&str>) -> RoomContext {
        RoomContext {
            parcel: parcel.map(str::to_string),
            ..ctx(realm, Some("bafkreiabc"))
        }
    }

    fn genesis_place(positions: &[&str]) -> PlaceInfo {
        PlaceInfo {
            world: false,
            world_name: None,
            positions: positions.iter().map(|p| p.to_string()).collect(),
            base_position: positions.first().map(|p| p.to_string()),
        }
    }

    fn world_place(name: &str) -> PlaceInfo {
        PlaceInfo {
            world: true,
            world_name: Some(name.into()),
            positions: vec!["0,0".into()],
            base_position: Some("0,0".into()),
        }
    }

    #[test]
    fn the_room_plan_matches_upstream_get_room_name() {
        assert_eq!(
            plan(&ctx(Some("main"), Some("bafkreiabc"))),
            RoomPlan::Scene("scene:main:bafkreiabc".into())
        );
        assert_eq!(
            plan(&ctx(Some("foo.dcl.eth"), Some("bafkreiabc"))),
            RoomPlan::World {
                world: "foo.dcl.eth".into(),
                scene_id: Some("bafkreiabc".into())
            }
        );
        assert_eq!(
            plan(&ctx(Some("foo.dcl.eth"), None)),
            RoomPlan::World {
                world: "foo.dcl.eth".into(),
                scene_id: None
            }
        );
        for realm in ["localpreview", "preview", "LocalPreview", "PREVIEW"] {
            assert_eq!(
                plan(&ctx(Some(realm), Some("b64-scene"))),
                RoomPlan::Scene(format!("scene:{realm}:b64-scene"))
            );
        }
        assert_eq!(plan(&ctx(Some("main"), None)), RoomPlan::Skip);
        assert_eq!(plan(&ctx(None, Some("bafkreiabc"))), RoomPlan::FromPlace);
        assert_eq!(plan(&ctx(None, None)), RoomPlan::FromPlace);
    }

    #[test]
    fn a_context_must_stand_on_the_authorized_place() {
        assert!(context_matches_place(
            &ctx_at(Some("Foo.dcl.eth"), None),
            &world_place("foo.dcl.eth")
        ));
        assert!(!context_matches_place(
            &ctx_at(Some("other.dcl.eth"), None),
            &world_place("foo.dcl.eth")
        ));
        assert!(!context_matches_place(
            &ctx_at(Some("foo.dcl.eth"), Some("10,20")),
            &genesis_place(&["10,20"])
        ));
        assert!(context_matches_place(
            &ctx_at(Some("main"), Some("10,21")),
            &genesis_place(&["10,20", "10,21"])
        ));
        assert!(!context_matches_place(
            &ctx_at(Some("main"), Some("99,99")),
            &genesis_place(&["10,20", "10,21"])
        ));
        assert!(!context_matches_place(
            &ctx_at(Some("main"), None),
            &genesis_place(&["10,20"])
        ));
        assert!(!context_matches_place(
            &ctx_at(Some("main"), Some("0,0")),
            &world_place("foo.dcl.eth")
        ));
        assert!(context_matches_place(
            &ctx_at(None, None),
            &genesis_place(&["10,20"])
        ));
    }

    #[test]
    fn context_reads_the_realm_and_scene_upstream_signs_over() {
        let flat = RoomContext::from_metadata(
            &json!({ "realmName": "main", "sceneId": "bafkreiabc" }),
            "place-1",
        );
        assert_eq!(plan(&flat), RoomPlan::Scene("scene:main:bafkreiabc".into()));
        let nested = RoomContext::from_metadata(
            &json!({ "realm": { "serverName": "main" }, "sceneId": "bafkreiabc" }),
            "place-1",
        );
        assert_eq!(
            plan(&nested),
            RoomPlan::Scene("scene:main:bafkreiabc".into())
        );
        assert_eq!(
            plan(&RoomContext::from_metadata(&json!({}), "place-1")),
            RoomPlan::FromPlace
        );
        let with_parcel = RoomContext::from_metadata(
            &json!({ "realmName": "main", "sceneId": "bafkreiabc", "parcel": "10,20" }),
            "place-1",
        );
        assert_eq!(with_parcel.parcel.as_deref(), Some("10,20"));
    }

    #[test]
    fn listing_key_and_place_from_metadata() {
        assert_eq!(
            listing_key_candidate(&json!({ "realmName": "foo.dcl.eth", "sceneId": "bafkreiabc" }))
                .as_deref(),
            Some("bafkreiabc")
        );
        assert_eq!(
            listing_key_candidate(&json!({ "realmName": "foo.dcl.eth", "sceneId": "foo.dcl.eth" }))
                .as_deref(),
            Some("foo.dcl.eth")
        );
        assert_eq!(
            listing_key_candidate(&json!({ "realmName": "foo.dcl.eth" })).as_deref(),
            Some("foo.dcl.eth")
        );
        assert_eq!(
            listing_key_candidate(&json!({ "realm": { "serverName": "bar.eth" } })).as_deref(),
            Some("bar.eth")
        );
        assert_eq!(
            listing_key_candidate(&json!({ "realmName": "main", "sceneId": "bafkreixyz" }))
                .as_deref(),
            Some("bafkreixyz")
        );
        assert_eq!(listing_key_candidate(&json!({})), None);
        assert_eq!(listing_key_candidate(&json!({ "realmName": "main" })), None);
        assert_eq!(
            place_from_metadata(&json!({ "realmName": "foo.dcl.eth", "sceneId": "x" })).as_deref(),
            Some("foo.dcl.eth")
        );
        assert_eq!(
            place_from_metadata(&json!({ "realmName": "main", "sceneId": "x" })).as_deref(),
            Some("x")
        );
    }

    /// The lease document is read only from `LEASE_AUTHORIZATIONS_URL`
    /// (the mirror posture's): without one no holder is listed and nothing
    /// is fetched.
    #[tokio::test]
    async fn lease_holders_come_only_from_the_configured_document() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let stub = axum::Router::new().route(
            "/authorizations.json",
            axum::routing::get(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                async {
                    axum::Json(json!([
                        { "addresses": ["0xAAAAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"], "plots": ["1,2"] },
                        { "addresses": ["0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"], "plots": ["9,9"] }
                    ]))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });
        let state_with = |url: Option<String>| {
            let settings = crate::gatekeeper::Settings {
                lease_authorizations_url: url,
                ..crate::gatekeeper::Settings::for_tests()
            };
            let pool = sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres")
                .expect("a lazy pool never connects");
            CommsState::new(
                pool,
                crate::http::Outbound::for_tests(),
                settings,
                None,
                None,
                Arc::new(StubPolicy::default()),
                Arc::new(StubScenes::default()),
                Arc::new(crate::presence::Presence::new()),
                Arc::new(crate::interfaces::test_support::StubDirectory::default()),
            )
        };
        let parcels = vec!["1,2".to_string(), "3,4".to_string()];

        let off = state_with(None);
        assert!(
            get_lease_holders_for_parcels(&off, &parcels)
                .await
                .is_empty()
        );
        assert_eq!(hits.load(Ordering::SeqCst), 0, "no document is fetched");

        let on = state_with(Some(format!("http://{addr}/authorizations.json")));
        assert_eq!(
            get_lease_holders_for_parcels(&on, &parcels).await,
            BTreeSet::from(["0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()])
        );
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    fn adapter_state(pool: sqlx::PgPool, policy: StubPolicy) -> CommsState {
        CommsState::new(
            pool,
            crate::http::Outbound::for_tests(),
            crate::gatekeeper::Settings::for_tests(),
            None,
            None,
            Arc::new(policy),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(crate::interfaces::test_support::StubDirectory::default()),
        )
    }

    #[tokio::test]
    async fn the_scene_adapter_refuses_invalid_worlds() {
        use crate::access::WorldStanding;
        let actor = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        for standing in [
            WorldStanding::Unknown,
            WorldStanding::Disabled,
            WorldStanding::DenyListed,
        ] {
            let state = adapter_state(
                sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
                StubPolicy {
                    standing: Ok(standing),
                    ..Default::default()
                },
            );
            assert!(
                !has_world_access_permission(
                    &state,
                    &actor,
                    actor.as_str(),
                    "alice.dcl.eth",
                    None,
                    None,
                    None
                )
                .await
                .unwrap(),
                "{standing:?}"
            );
        }
        let state = adapter_state(
            sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
            StubPolicy {
                standing: Err(AuthorityNotEstablished::store_unavailable(
                    "worlds.worlds",
                    &sqlx::Error::PoolTimedOut,
                )),
                ..Default::default()
            },
        );
        let error = has_world_access_permission(
            &state,
            &actor,
            actor.as_str(),
            "alice.dcl.eth",
            None,
            None,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, 503);
    }

    #[tokio::test]
    async fn the_scene_adapter_spends_the_shared_secret_budget() {
        use crate::access::{AccessType, WorldStanding};
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("adapter-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        let actor = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let caller = format!("caller-{}", uuid::Uuid::new_v4().simple());
        let state = |access| {
            adapter_state(
                pool.clone(),
                StubPolicy {
                    standing: Ok(WorldStanding::Open(AccessType::SharedSecret)),
                    access: Ok(access),
                    ..Default::default()
                },
            )
            .with_shared_secret_max_attempts(2)
        };
        let check = |state: CommsState| {
            let (world, caller, actor) = (world.clone(), caller.clone(), actor.clone());
            async move {
                has_world_access_permission(
                    &state,
                    &actor,
                    &caller,
                    &world,
                    None,
                    Some("guess"),
                    None,
                )
                .await
                .map_err(|error| error.code)
            }
        };
        assert_eq!(
            check(state(Access::Allowed(
                crate::access::AccessGrant::SharedSecret
            )))
            .await,
            Ok(true)
        );
        for _ in 0..2 {
            assert_eq!(check(state(Access::InvalidSecret)).await, Ok(false));
        }
        assert_eq!(check(state(Access::InvalidSecret)).await, Err(429));
        assert_eq!(
            check(state(Access::Allowed(
                crate::access::AccessGrant::SharedSecret
            )))
            .await,
            Err(429),
            "the budget is spent before the secret is checked"
        );
    }

    #[tokio::test]
    async fn the_scene_adapter_refunds_correct_secrets() {
        use crate::access::{AccessType, WorldStanding};
        let Some(pool) = crate::db::scratch::pool(8).await else {
            return;
        };
        let world = format!("adapter-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        let actor = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let caller = format!("caller-{}", uuid::Uuid::new_v4().simple());
        let state = |access| {
            adapter_state(
                pool.clone(),
                StubPolicy {
                    standing: Ok(WorldStanding::Open(AccessType::SharedSecret)),
                    access: Ok(access),
                    ..Default::default()
                },
            )
            .with_shared_secret_max_attempts(2)
        };
        let check = |state: CommsState| {
            let (world, caller, actor) = (world.clone(), caller.clone(), actor.clone());
            async move {
                has_world_access_permission(
                    &state,
                    &actor,
                    &caller,
                    &world,
                    None,
                    Some("guess"),
                    None,
                )
                .await
                .map_err(|error| error.code)
            }
        };
        let right = || state(Access::Allowed(crate::access::AccessGrant::SharedSecret));
        let parallel = futures::future::join_all((0..8).map(|_| check(right()))).await;
        assert!(parallel.iter().all(|r| *r == Ok(true)), "{parallel:?}");
        assert_eq!(check(state(Access::InvalidSecret)).await, Ok(false));
        assert_eq!(check(right()).await, Ok(true));
        assert_eq!(check(state(Access::InvalidSecret)).await, Ok(false));
        assert_eq!(
            check(right()).await,
            Err(429),
            "the success between failures kept them"
        );
    }
}
