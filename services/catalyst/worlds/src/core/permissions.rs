//! The world permission routes: the public `GET /world/{name}/permissions`
//! document and the owner-gated grant, parcel and access-list edits. The
//! document's `owner` is `Policy::current_owner_or_stored` for a world with
//! a row (the live NAME holder, the stored owner on a reader fault) and
//! only the personal rule otherwise. Every owner question of an edit is
//! `Policy::is_owner` (personal world, then the name authority,
//! `worlds.owner` only when no reader is configured; an unanswerable
//! authority is a 503, never a silent refusal); the grant writes are
//! `store::*` over the PK-keyed tables.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::comms::world_identity::{ParcelError, ParcelSet, WorldName};
use crate::policy::Permission;
use crate::shared::identity::{Address, PermissionDecision};
use crate::shared::signed_fetch::SignedPath;

use super::auth::{PERMISSIONS_METADATA_KEYS, map_auth_error_api, require_verified};
use super::http::ApiError;
use super::store::{
    self, AccessSetting, AllowListEdit, AllowListEditOutcome, MAX_ACCESS_COMMUNITIES,
    MAX_ACCESS_WALLETS,
};
use super::{Revocation, SharedState};

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct AllowListPermission {
    #[serde(rename = "type")]
    pub kind: String,
    pub wallets: Vec<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct WorldPermissionsBlock {
    pub deployment: AllowListPermission,
    pub streaming: AllowListPermission,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub access: Value,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct PermissionSummaryEntry {
    pub permission: String,
    pub world_wide: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "number | null", optional))]
    pub parcel_count: Option<i64>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct PermissionsResponse {
    pub permissions: WorldPermissionsBlock,
    pub owner: Option<String>,
    pub summary: BTreeMap<String, Vec<PermissionSummaryEntry>>,
}

pub(super) fn parse_world(raw: &str) -> Result<WorldName, ApiError> {
    WorldName::parse(raw).map_err(|e| ApiError::bad_request(format!("Invalid world name: {e}")))
}

fn parse_address(raw: &str) -> Result<Address, ApiError> {
    Address::parse(raw).map_err(|_| ApiError::bad_request(format!("Invalid address: {raw}.")))
}

/// GET /world/{world_name}/permissions
pub async fn get_permissions(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
) -> Result<Json<PermissionsResponse>, ApiError> {
    let world = parse_world(&world_name)?;
    let record = store::get_world(&state.pool, &world).await?;
    let records = store::permission_records(&state.pool, &world).await?;
    let access = record
        .as_ref()
        .map(|w| w.access.clone())
        .unwrap_or_default();
    // No subgraph lookup without a row: this route is unauthenticated.
    let owner = match &record {
        Some(w) => {
            state
                .policy
                .current_owner_or_stored(&world, w.owner.clone())
                .await
        }
        None => state.policy.personal_worlds().owner_of(&world),
    };

    let mut deployment_wallets = Vec::new();
    let mut streaming_wallets = Vec::new();
    let mut summary: BTreeMap<String, Vec<PermissionSummaryEntry>> = BTreeMap::new();
    for r in &records {
        match r.permission.as_str() {
            "deployment" => deployment_wallets.push(r.address.clone()),
            "streaming" => streaming_wallets.push(r.address.clone()),
            _ => {}
        }
        summary
            .entry(r.address.clone())
            .or_default()
            .push(PermissionSummaryEntry {
                permission: r.permission.clone(),
                world_wide: r.is_world_wide,
                parcel_count: if r.is_world_wide {
                    None
                } else {
                    Some(r.parcel_count)
                },
            });
    }

    Ok(Json(PermissionsResponse {
        permissions: WorldPermissionsBlock {
            deployment: AllowListPermission {
                kind: "allow-list".to_string(),
                wallets: deployment_wallets,
            },
            streaming: AllowListPermission {
                kind: "allow-list".to_string(),
                wallets: streaming_wallets,
            },
            access: access.to_public_json(),
        },
        owner: owner.map(|a| a.into_string()),
        summary,
    }))
}

struct OwnerCheck {
    signer: Address,
    /// Whether the `worlds` row already exists, so writers can skip creating it.
    world_found: bool,
}

fn not_owner(world: &WorldName) -> ApiError {
    ApiError::forbidden(format!(
        "Your wallet does not own \"{}\", you can not set access control lists for it.",
        world.display()
    ))
}

async fn owner_decision(
    state: &SharedState,
    world: &WorldName,
    signer: &Address,
) -> Result<(), ApiError> {
    match state.policy.is_owner(world, signer).await {
        PermissionDecision::Allowed => Ok(()),
        PermissionDecision::Refused(
            crate::shared::identity::AuthorityNotEstablished::RefusedLacksAuthority { .. },
        ) => Err(not_owner(world)),
        PermissionDecision::Refused(undetermined) => Err(undetermined.into()),
    }
}

async fn verify_owner(
    state: &SharedState,
    headers: &HeaderMap,
    path: &SignedPath,
    method: &str,
    world: &WorldName,
) -> Result<OwnerCheck, ApiError> {
    let auth = require_verified(headers, method, path, &[], state.eip1654.as_deref())
        .await
        .map_err(map_auth_error_api)?;
    owner_decision(state, world, &auth.signer).await?;
    let world_found = store::get_world(&state.pool, world).await?.is_some();
    Ok(OwnerCheck {
        signer: auth.signer,
        world_found,
    })
}

fn parse_permission(raw: &str) -> Option<Permission> {
    Permission::parse(raw)
}

/// POST /world/{world_name}/permissions/{permission_name}
pub async fn post_permissions(
    State(state): State<SharedState>,
    Path((world_name, permission_name)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let world = parse_world(&world_name)?;
    let auth = require_verified(
        &headers,
        "post",
        &SignedPath::from_uris(&original, &uri),
        PERMISSIONS_METADATA_KEYS,
        state.eip1654.as_deref(),
    )
    .await
    .map_err(map_auth_error_api)?;
    owner_decision(&state, &world, &auth.signer).await?;

    let meta = &auth.metadata;
    match permission_name.as_str() {
        "deployment" => {
            let ty = meta.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if ty != "allow-list" {
                return Err(ApiError::bad_request(
                    "Invalid payload received. Deployment permission needs to be 'allow-list'.",
                ));
            }
            let wallets = metadata_wallets(meta)?;
            store::replace_world_wide_permission(
                &state.pool,
                &world,
                &auth.signer,
                Permission::Deployment,
                &wallets,
            )
            .await?;
        }
        "streaming" => {
            let ty = meta.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if ty != "unrestricted" && ty != "allow-list" {
                return Err(ApiError::bad_request(
                    "Invalid payload received. Streaming permission needs to be either 'unrestricted' or 'allow-list'.",
                ));
            }
            let wallets = if ty == "unrestricted" {
                Vec::new()
            } else {
                metadata_wallets(meta)?
            };
            store::replace_world_wide_permission(
                &state.pool,
                &world,
                &auth.signer,
                Permission::Streaming,
                &wallets,
            )
            .await?;
        }
        "access" => {
            let access = access_from_metadata(meta)?;
            if let AccessSetting::AllowList { communities, .. } = &access {
                require_community_member(&state, &auth.signer, communities).await?;
            }
            store::store_access_for_owner(&state.pool, &world, &auth.signer, &access).await?;
            state
                .revoked(Revocation::AccessSetting {
                    world: world.clone(),
                })
                .await;
        }
        other => {
            return Err(ApiError::bad_request(format!(
                "Invalid permission name: {other}."
            )));
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

fn metadata_strings(meta: &Value, key: &str) -> Vec<String> {
    meta.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|w| w.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn metadata_wallets(meta: &Value) -> Result<Vec<Address>, ApiError> {
    metadata_strings(meta, "wallets")
        .iter()
        .map(|w| parse_address(w))
        .collect()
}

fn access_from_metadata(meta: &Value) -> Result<AccessSetting, ApiError> {
    let ty = meta.get("type").and_then(|v| v.as_str()).unwrap_or("");
    Ok(match ty {
        "unrestricted" => AccessSetting::Unrestricted,
        "nft-ownership" => {
            let nft = meta.get("nft").and_then(|v| v.as_str()).ok_or_else(|| {
                ApiError::bad_request("For nft ownership there needs to be a valid nft.")
            })?;
            AccessSetting::NftOwnership {
                nft: nft.to_string(),
            }
        }
        "shared-secret" => {
            let secret = meta
                .get("secret")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    ApiError::bad_request("For shared secret there needs to be a valid secret.")
                })?;
            let hash = bcrypt::hash(secret, bcrypt::DEFAULT_COST)
                .map_err(|e| ApiError::internal(format!("hash secret: {e}")))?;
            AccessSetting::SharedSecret { secret_hash: hash }
        }
        "allow-list" => {
            let wallets = metadata_strings(meta, "wallets");
            let communities = metadata_strings(meta, "communities");
            if wallets.len() > MAX_ACCESS_WALLETS as usize {
                return Err(ApiError::bad_request(format!(
                    "Too many wallets in allow-list. Maximum allowed is {MAX_ACCESS_WALLETS}, but {} were provided.",
                    wallets.len()
                )));
            }
            if communities.len() > MAX_ACCESS_COMMUNITIES as usize {
                return Err(ApiError::bad_request(format!(
                    "Too many communities. Maximum allowed is {MAX_ACCESS_COMMUNITIES}, but {} were provided.",
                    communities.len()
                )));
            }
            for w in &wallets {
                parse_address(w)?;
            }
            for c in &communities {
                parse_community(c)?;
            }
            AccessSetting::AllowList {
                wallets,
                communities,
            }
        }
        other => {
            return Err(ApiError::bad_request(format!(
                "Invalid access type: {other}."
            )));
        }
    })
}

/// The signer must belong to every community it lists (the old
/// `UnauthorizedCommunityError`, a 400).
async fn require_community_member(
    state: &SharedState,
    signer: &Address,
    communities: &[String],
) -> Result<(), ApiError> {
    if communities.is_empty() {
        return Ok(());
    }
    let ids = communities
        .iter()
        .map(|c| parse_community(c))
        .collect::<Result<Vec<_>, _>>()?;
    let member = state.policy.member_communities(signer, &ids).await?;
    let missing: Vec<&str> = communities
        .iter()
        .zip(&ids)
        .filter(|(_, id)| !member.contains(id))
        .map(|(raw, _)| raw.as_str())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(ApiError::bad_request(format!(
        "You are not a member of the following communities: {}.",
        missing.join(", ")
    )))
}

/// `worlds.world_access_communities.community_id` is a uuid (the social
/// communities' key); the old service stored any string.
fn parse_community(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw.trim()).map_err(|_| ApiError::bad_request("Invalid community id."))
}

fn is_allow_list_permission(p: Permission) -> bool {
    p.supports_parcels()
}

/// PUT /world/{world_name}/permissions/{permission_name}/{address}
pub async fn put_permissions_address(
    State(state): State<SharedState>,
    Path((world_name, permission_name, address)): Path<(String, String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let address = parse_address(&address)?;
    let Some(permission) = parse_permission(&permission_name) else {
        return Err(ApiError::bad_request(format!(
            "Invalid permission name: {permission_name}."
        )));
    };
    let world = parse_world(&world_name)?;
    let owner = verify_owner(
        &state,
        &headers,
        &SignedPath::from_uris(&original, &uri),
        "put",
        &world,
    )
    .await?;
    let ensure_owner = (!owner.world_found).then_some(&owner.signer);

    if is_allow_list_permission(permission) {
        store::grant_addresses_world_wide_permission(
            &state.pool,
            &world,
            permission,
            std::slice::from_ref(&address),
            ensure_owner,
        )
        .await?;
    } else {
        if let Some(signer) = ensure_owner {
            store::ensure_world(&state.pool, &world, signer).await?;
        }
        apply_allow_list_edit(&state, &world, AllowListEdit::AddWallet(&address), || {
            ApiError::bad_request(format!("Cannot add wallet: allow-list would exceed the maximum of {MAX_ACCESS_WALLETS} wallets."))
        })
        .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /world/{world_name}/permissions/{permission_name}/{address}
pub async fn delete_permissions_address(
    State(state): State<SharedState>,
    Path((world_name, permission_name, address)): Path<(String, String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let address = parse_address(&address)?;
    let Some(permission) = parse_permission(&permission_name) else {
        return Err(ApiError::bad_request(format!(
            "Permission '{permission_name}' does not support allow-list. Only 'deployment', 'streaming', and 'access' do."
        )));
    };
    let world = parse_world(&world_name)?;
    verify_owner(
        &state,
        &headers,
        &SignedPath::from_uris(&original, &uri),
        "delete",
        &world,
    )
    .await?;
    if !store::world_is_valid(&state.pool, &world).await? {
        return Err(ApiError::not_found(format!(
            "World \"{}\" not found.",
            world.display()
        )));
    }

    if is_allow_list_permission(permission) {
        store::remove_addresses_permission(
            &state.pool,
            &world,
            permission,
            std::slice::from_ref(&address),
        )
        .await?;
    } else {
        apply_allow_list_edit(
            &state,
            &world,
            AllowListEdit::RemoveWallet(&address),
            || ApiError::internal("unreachable: removals have no cap"),
        )
        .await?;
        // Kicks only if nothing else admits it (e.g. a listed community).
        state.revoke_unless_admitted(&world, &address).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

const MAX_PARCELS_PER_REQUEST: usize = 500;

/// Upstream's `permissionParcelsSchema`: exactly `{parcels}`, 1-500 `x,y` strings.
fn parcels_body(body: Result<Json<Value>, JsonRejection>) -> Result<ParcelSet, ApiError> {
    // 415 mirrors upstream's schema validator; storage routes predate it and answer 400.
    let Json(value) = body.map_err(|rejection| match rejection {
        JsonRejection::MissingJsonContentType(_) => {
            ApiError::http(415, "Content-Type must be application/json")
        }
        other => ApiError::bad_request(other.body_text()),
    })?;
    schema_parcels(&value)
}

fn schema_parcels(value: &Value) -> Result<ParcelSet, ApiError> {
    let invalid = || ApiError::bad_request("Invalid JSON body");
    let body = value
        .as_object()
        .filter(|o| o.len() == 1)
        .ok_or_else(invalid)?;
    let items = body
        .get("parcels")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if !(1..=MAX_PARCELS_PER_REQUEST).contains(&items.len()) {
        return Err(invalid());
    }
    let raw = items
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    ParcelSet::parse_strict(&raw).map_err(|e| match e {
        ParcelError::NotStrict(_) => invalid(),
        ParcelError::Malformed(_) => ApiError::bad_request(format!("Invalid parcels: {e}")),
    })
}

fn validate_address_and_allow_list(
    address: &str,
    permission_name: &str,
) -> Result<(Address, Permission), ApiError> {
    let address = parse_address(address)?;
    match parse_permission(permission_name) {
        Some(p) if is_allow_list_permission(p) => Ok((address, p)),
        _ => Err(ApiError::bad_request(format!(
            "Permission '{permission_name}' does not support allow-list. Only 'deployment' and 'streaming' do."
        ))),
    }
}

/// POST /world/{world_name}/permissions/{permission_name}/address/{address}/parcels
pub async fn post_permission_parcels(
    State(state): State<SharedState>,
    Path((world_name, permission_name, address)): Path<(String, String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let parcels = parcels_body(body)?;
    let (address, permission) = validate_address_and_allow_list(&address, &permission_name)?;
    let world = parse_world(&world_name)?;
    verify_owner(
        &state,
        &headers,
        &SignedPath::from_uris(&original, &uri),
        "post",
        &world,
    )
    .await?;
    store::add_parcels_to_permission(&state.pool, &world, permission, &address, &parcels).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /world/{world_name}/permissions/{permission_name}/address/{address}/parcels
pub async fn delete_permission_parcels(
    State(state): State<SharedState>,
    Path((world_name, permission_name, address)): Path<(String, String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let parcels = parcels_body(body)?;
    let (address, permission) = validate_address_and_allow_list(&address, &permission_name)?;
    let world = parse_world(&world_name)?;
    verify_owner(
        &state,
        &headers,
        &SignedPath::from_uris(&original, &uri),
        "delete",
        &world,
    )
    .await?;
    let found =
        store::remove_parcels_from_permission(&state.pool, &world, permission, &address, &parcels)
            .await?;
    if !found {
        return Err(ApiError::bad_request(format!(
            "Permission not found. Address {} does not have {permission_name} permission for world {}.",
            address.as_str(),
            world.display()
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

type QueryPairs = Vec<(String, String)>;

fn query_param<'a>(q: &'a QueryPairs, key: &str) -> Option<&'a str> {
    q.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// JS `parseInt(s, 10)`: leading whitespace, an optional sign, then the
/// leading digits (saturating); `None` where JS gives `NaN`.
fn js_parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let (negative, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits = &rest[..rest.bytes().take_while(u8::is_ascii_digit).count()];
    if digits.is_empty() {
        return None;
    }
    let n = digits.parse::<i64>().unwrap_or(i64::MAX);
    Some(if negative { -n } else { n })
}

const MAX_PAGE_LIMIT: i64 = 100;

/// `getPaginationParams` of @dcl/http-commons: an absent, invalid or
/// out-of-range limit is 100; an invalid or negative offset is 0.
fn pagination(q: &QueryPairs) -> (i64, i64) {
    let limit = query_param(q, "limit")
        .and_then(js_parse_int)
        .filter(|l| (1..=MAX_PAGE_LIMIT).contains(l))
        .unwrap_or(MAX_PAGE_LIMIT);
    let offset = query_param(q, "offset")
        .and_then(js_parse_int)
        .filter(|o| *o >= 0)
        .unwrap_or(0);
    (limit, offset)
}

fn bounding_box(q: &QueryPairs) -> Result<Option<(i32, i32, i32, i32)>, ApiError> {
    let params = ["x1", "y1", "x2", "y2"].map(|k| query_param(q, k));
    if params.iter().all(Option::is_none) {
        return Ok(None);
    }
    let [Some(x1), Some(y1), Some(x2), Some(y2)] = params else {
        return Err(ApiError::bad_request(
            "Bounding box requires all four parameters: x1, y1, x2, y2.",
        ));
    };
    let coordinate = |raw: &str| {
        js_parse_int(raw)
            .map(|n| n.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
            .ok_or_else(|| ApiError::bad_request("Bounding box parameters must be valid integers."))
    };
    Ok(Some((
        coordinate(x1)?,
        coordinate(y1)?,
        coordinate(x2)?,
        coordinate(y2)?,
    )))
}

/// GET /world/{world_name}/permissions/{permission_name}/address/{address}/parcels
pub async fn get_allowed_parcels_for_permission(
    State(state): State<SharedState>,
    Path((world_name, permission_name, address)): Path<(String, String, String)>,
    Query(q): Query<QueryPairs>,
) -> Result<Json<Value>, ApiError> {
    let (address, permission) = validate_address_and_allow_list(&address, &permission_name)?;
    let world = parse_world(&world_name)?;
    let (limit, offset) = pagination(&q);
    let bbox = bounding_box(&q)?;
    let (total, parcels) = store::parcels_for_permission(
        &state.pool,
        &world,
        permission,
        &address,
        limit,
        offset,
        bbox,
    )
    .await?
    .ok_or_else(|| {
        ApiError::not_found(format!(
            "Permission '{permission_name}' not found for address {} in world {}.",
            address.as_str(),
            world.display()
        ))
    })?;
    Ok(Json(json!({ "total": total, "parcels": parcels })))
}

/// POST /world/{world_name}/permissions/{permission_name}/parcels
pub async fn get_addresses_for_parcel_permission(
    State(state): State<SharedState>,
    Path((world_name, permission_name)): Path<(String, String)>,
    Query(q): Query<QueryPairs>,
    body: Result<Json<Value>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let parcels = parcels_body(body)?;
    let permission = match parse_permission(&permission_name) {
        Some(p) if is_allow_list_permission(p) => p,
        _ => {
            return Err(ApiError::bad_request(format!(
                "Permission '{permission_name}' does not support allow-list. Only 'deployment' and 'streaming' do."
            )));
        }
    };
    let world = parse_world(&world_name)?;
    let (limit, offset) = pagination(&q);
    let (total, addresses) = store::addresses_for_parcel_permission(
        &state.pool,
        &world,
        permission,
        &parcels,
        limit,
        offset,
    )
    .await?;
    Ok(Json(json!({ "total": total, "addresses": addresses })))
}

/// PUT /world/{world_name}/permissions/access/communities/{communityId}
pub async fn put_permissions_access_community(
    State(state): State<SharedState>,
    Path((world_name, community_id)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let community = parse_community(&community_id)?;
    let world = parse_world(&world_name)?;
    let owner = verify_owner(
        &state,
        &headers,
        &SignedPath::from_uris(&original, &uri),
        "put",
        &world,
    )
    .await?;
    require_community_member(&state, &owner.signer, &[community_id]).await?;
    apply_allow_list_edit(&state, &world, AllowListEdit::AddCommunity(community), || {
        ApiError::bad_request(format!("Too many communities. Maximum allowed is {MAX_ACCESS_COMMUNITIES}, cannot add more."))
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /world/{world_name}/permissions/access/communities/{communityId}
pub async fn delete_permissions_access_community(
    State(state): State<SharedState>,
    Path((world_name, community_id)): Path<(String, String)>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let community = parse_community(&community_id)?;
    let world = parse_world(&world_name)?;
    verify_owner(
        &state,
        &headers,
        &SignedPath::from_uris(&original, &uri),
        "delete",
        &world,
    )
    .await?;
    apply_allow_list_edit(
        &state,
        &world,
        AllowListEdit::RemoveCommunity(community),
        || ApiError::internal("unreachable: removals have no cap"),
    )
    .await?;
    state
        .revoked(Revocation::AccessSetting {
            world: world.clone(),
        })
        .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn apply_allow_list_edit(
    state: &SharedState,
    world: &WorldName,
    edit: AllowListEdit<'_>,
    cap_error: impl FnOnce() -> ApiError,
) -> Result<(), ApiError> {
    match store::modify_allow_list_access(&state.pool, world, edit).await? {
        AllowListEditOutcome::Applied => Ok(()),
        AllowListEditOutcome::NotAllowList => Err(ApiError::bad_request(format!(
            "World \"{}\" does not have allow-list access type.",
            world.display()
        ))),
        AllowListEditOutcome::CapExceeded => Err(cap_error()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_metadata_parses_the_old_document_shapes() {
        assert_eq!(
            access_from_metadata(&json!({ "type": "unrestricted" })).unwrap(),
            AccessSetting::Unrestricted
        );
        assert_eq!(
            access_from_metadata(&json!({ "type": "nft-ownership", "nft": "urn:x" })).unwrap(),
            AccessSetting::NftOwnership {
                nft: "urn:x".into()
            }
        );
        assert_eq!(
            access_from_metadata(&json!({ "type": "nft-ownership" }))
                .unwrap_err()
                .status(),
            400
        );
        assert_eq!(
            access_from_metadata(&json!({ "type": "shared-secret", "secret": "" }))
                .unwrap_err()
                .status(),
            400
        );
        assert!(matches!(
            access_from_metadata(&json!({ "type": "shared-secret", "secret": "MySecret" })).unwrap(),
            AccessSetting::SharedSecret { secret_hash } if bcrypt::verify("MySecret", &secret_hash).unwrap()
        ));
        let id = Uuid::new_v4().to_string();
        assert_eq!(
            access_from_metadata(&json!({ "type": "allow-list", "wallets": ["0xAbC0000000000000000000000000000000000001"], "communities": [id] })).unwrap(),
            AccessSetting::AllowList { wallets: vec!["0xAbC0000000000000000000000000000000000001".into()], communities: vec![id.clone()] }
        );
        assert_eq!(
            access_from_metadata(&json!({ "type": "allow-list", "wallets": ["zzz"] }))
                .unwrap_err()
                .status(),
            400
        );
        assert_eq!(
            access_from_metadata(&json!({ "type": "allow-list", "communities": ["not-a-uuid"] }))
                .unwrap_err()
                .status(),
            400
        );
        assert_eq!(
            access_from_metadata(&json!({ "type": "bogus" }))
                .unwrap_err()
                .status(),
            400
        );
        let many: Vec<String> = (0..1001).map(|i| format!("0x{i:040x}")).collect();
        assert!(
            access_from_metadata(&json!({ "type": "allow-list", "wallets": many }))
                .unwrap_err()
                .public_message()
                .contains("Too many wallets")
        );
    }

    #[test]
    fn route_parameter_validation_matches_the_old_messages() {
        assert_eq!(
            validate_address_and_allow_list("zzz", "deployment")
                .unwrap_err()
                .public_message(),
            "Invalid address: zzz."
        );
        assert!(
            validate_address_and_allow_list("0x0000000000000000000000000000000000000001", "access")
                .unwrap_err()
                .public_message()
                .contains("Only 'deployment' and 'streaming' do.")
        );
        assert!(
            validate_address_and_allow_list(
                "0x0000000000000000000000000000000000000001",
                "streaming"
            )
            .is_ok()
        );
        assert!(parse_community(" ").is_err());
    }

    fn pairs(raw: &[(&str, &str)]) -> QueryPairs {
        raw.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn pagination_falls_back_like_http_commons() {
        assert_eq!(pagination(&pairs(&[])), (100, 0));
        assert_eq!(
            pagination(&pairs(&[("limit", "5"), ("offset", "7")])),
            (5, 7)
        );
        assert_eq!(pagination(&pairs(&[("limit", "1000")])), (100, 0));
        assert_eq!(pagination(&pairs(&[("limit", "0")])), (100, 0));
        assert_eq!(
            pagination(&pairs(&[("limit", "-1"), ("offset", "-3")])),
            (100, 0)
        );
        assert_eq!(
            pagination(&pairs(&[("limit", "abc"), ("offset", "x")])),
            (100, 0)
        );
        assert_eq!(
            pagination(&pairs(&[("limit", ""), ("offset", "")])),
            (100, 0)
        );
        assert_eq!(
            pagination(&pairs(&[("limit", "7.9"), ("offset", "3px")])),
            (7, 3)
        );
        assert_eq!(
            pagination(&pairs(&[("limit", "2"), ("limit", "50")])),
            (2, 0)
        );
    }

    #[test]
    fn bounding_box_parses_like_parse_int() {
        assert_eq!(bounding_box(&pairs(&[])).unwrap(), None);
        let full = |x1| pairs(&[("x1", x1), ("y1", "0"), ("x2", "1"), ("y2", "-1")]);
        assert_eq!(bounding_box(&full("1.5")).unwrap(), Some((1, 0, 1, -1)));
        assert_eq!(
            bounding_box(&full("abc")).unwrap_err().public_message(),
            "Bounding box parameters must be valid integers."
        );
        assert_eq!(
            bounding_box(&full("")).unwrap_err().public_message(),
            "Bounding box parameters must be valid integers."
        );
        assert_eq!(
            bounding_box(&pairs(&[("x1", "0"), ("y1", "0")]))
                .unwrap_err()
                .public_message(),
            "Bounding box requires all four parameters: x1, y1, x2, y2."
        );
    }

    #[test]
    fn parcels_body_matches_the_upstream_schema() {
        assert_eq!(
            schema_parcels(&json!({ "parcels": ["0,0", "-1,-20", "00,01"] }))
                .unwrap()
                .canonical(),
            vec!["-1,-20".to_string(), "0,0".into(), "0,1".into()]
        );
        let too_many: Vec<String> = (0..501).map(|i| format!("{i},0")).collect();
        let max: Vec<String> = too_many[..500].to_vec();
        assert!(schema_parcels(&json!({ "parcels": max })).is_ok());
        for bad in [
            json!({ "parcels": [] }),
            json!({ "parcels": too_many }),
            json!({}),
            json!([]),
            json!({ "parcels": "0,0" }),
            json!({ "parcels": ["0,0"], "extra": true }),
            json!({ "parcels": [1] }),
            json!({ "parcels": ["0, 0"] }),
            json!({ "parcels": ["+1,0"] }),
            json!({ "parcels": ["1,2,3"] }),
            json!({ "parcels": ["1,"] }),
            json!({ "parcels": ["0,0\n"] }),
        ] {
            assert_eq!(
                schema_parcels(&bad).unwrap_err().public_message(),
                "Invalid JSON body",
                "{bad}"
            );
        }
        assert!(
            schema_parcels(&json!({ "parcels": ["99999999999,0"] }))
                .unwrap_err()
                .public_message()
                .starts_with("Invalid parcels:")
        );
    }
}
