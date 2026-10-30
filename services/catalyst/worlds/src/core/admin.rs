//! The bearer-gated operator routes (`CATALYRST_WORLDS_ADMIN_TOKEN`): the
//! world listing and detail, enable/disable, the wallet block list, the
//! access log, a world ban-status probe and `/gc` + `/admin/gc`. The old
//! `/admin/gc` walked the worlds contents directory and deleted every aged
//! file no world named; the ONE blob store also holds every other domain's
//! files, so the routes run the store-wide collector (`storage::gc`, every
//! domain's references) with the old query and answer shape. The age floor
//! is at least an hour and the upload window; like every POST here it is
//! behind `WORLDS_WRITES_ENABLED`, and it runs without the 30 s request
//! deadline (a full store walk).

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use chrono::SecondsFormat;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comms::world_identity::{Parcel, SceneId};
use crate::policy::Ban;
use crate::rooms;
use crate::shared::identity::Address;

use super::SharedState;
use super::auth::authorize_admin;
use super::http::{ApiError, clamp_limit};
use super::permissions::parse_world;
use super::store;

const DEFAULT_LIMIT: i64 = 100;
const MAX_LIMIT: i64 = 1000;

fn rfc3339(t: chrono::NaiveDateTime) -> String {
    t.and_utc().to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

fn clamp_page(q: &PageQuery) -> (i64, i64) {
    (
        clamp_limit(q.limit, DEFAULT_LIMIT, MAX_LIMIT),
        q.offset.unwrap_or(0).max(0),
    )
}

fn admin(state: &SharedState, headers: &HeaderMap) -> Result<(), ApiError> {
    authorize_admin(state.settings.admin_token.as_deref(), headers)
}

#[derive(Debug, Deserialize)]
pub struct GcQuery {
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub min_age_secs: Option<u64>,
    #[serde(default)]
    pub allow_empty_active_set: Option<bool>,
}

/// POST /gc, /admin/gc: the blob store collector.
pub async fn garbage_collect(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<GcQuery>,
) -> Result<Json<Value>, ApiError> {
    admin(&state, &headers)?;
    let limits = &state.settings.upload_limits;
    let floor = catalyst_storage::gc::orphan_age_floor(
        limits.multipart_upload_timeout_ms,
        limits.deployment_processing_timeout_ms,
    );
    let min_age = q
        .min_age_secs
        .map(std::time::Duration::from_secs)
        .unwrap_or(floor)
        .max(floor);
    let options = catalyst_storage::gc::GcOptions {
        dry_run: q.dry_run.unwrap_or(false),
        min_age,
        allow_empty_reference_set: q.allow_empty_active_set.unwrap_or(false),
    };
    let report = match catalyst_storage::gc::collect(
        &state.pool,
        &state.storage,
        &state.settings.global_scenes_urn,
        options,
    )
    .await
    {
        Ok(report) => report,
        Err(catalyst_storage::gc::GcError::EmptyReferenceSet) => {
            return Err(ApiError::bad_request(
                "Refusing to collect: no domain names any stored content, which usually means the database is empty or misconfigured. Re-run with allow_empty_active_set=true to collect anyway.",
            ));
        }
        Err(e) => {
            tracing::error!(error = %e, "garbage collection aborted; nothing was deleted");
            return Err(ApiError::internal(
                "Garbage collection aborted; nothing was deleted",
            ));
        }
    };
    let message = if report.dry_run {
        format!(
            "Garbage collection would remove {} unused keys.",
            report.candidates
        )
    } else {
        format!("Garbage collection removed {} unused keys.", report.removed)
    };
    Ok(Json(json!({
        "message": message,
        "dryRun": report.dry_run,
        "removed": report.removed,
        "failed": report.failed,
        "candidates": report.candidates,
        "scanned": report.scanned,
        "activeKeys": report.references,
        "minAgeSeconds": report.min_age_seconds,
    })))
}

/// GET /admin/worlds
pub async fn list_worlds(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    admin(&state, &headers)?;
    let (limit, offset) = clamp_page(&q);
    let (rows, total) = store::admin_list_worlds_page(&state.pool, limit, offset).await?;
    let worlds: Vec<Value> = rows
        .into_iter()
        .map(|w| {
            json!({
                "name": w.name,
                "owner": w.owner,
                "accessType": w.access_type,
                "blocked": w.blocked_since.is_some(),
                "blockedSince": w.blocked_since.map(rfc3339),
                "spawnCoordinates": w.spawn_coordinates,
                "sceneCount": w.scene_count,
            })
        })
        .collect();
    Ok(Json(
        json!({ "total": total, "limit": limit, "offset": offset, "worlds": worlds }),
    ))
}

/// GET /admin/worlds/{world_name}
pub async fn world_detail(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    admin(&state, &headers)?;
    let world = parse_world(&world_name)?;
    let (record, scenes, perms) = tokio::try_join!(
        store::get_world(&state.pool, &world),
        store::admin_scenes(&state.pool, &world),
        store::permission_records(&state.pool, &world),
    )?;
    let record = record
        .ok_or_else(|| ApiError::not_found(format!("World \"{world_name}\" was not found.")))?;
    let permissions: Vec<Value> = perms
        .into_iter()
        .map(|p| json!({ "address": p.address, "type": p.permission }))
        .collect();
    Ok(Json(json!({
        "name": record.name,
        "owner": record.owner.map(|a| a.into_string()),
        "access": record.access.to_public_json(),
        "blocked": record.blocked_since.is_some(),
        "blockedSince": record.blocked_since.map(rfc3339),
        "spawnCoordinates": record.spawn_coordinates,
        "sceneCount": scenes.len(),
        "scenes": scenes.iter().map(|(entity_id, parcels)| json!({ "entityId": entity_id, "parcels": parcels })).collect::<Vec<_>>(),
        "permissions": permissions,
    })))
}

/// POST /admin/worlds/{world_name}/disable
pub async fn disable_world(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    admin(&state, &headers)?;
    set_world_blocked(&state, &world_name, true).await
}

/// POST /admin/worlds/{world_name}/enable
pub async fn enable_world(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    admin(&state, &headers)?;
    set_world_blocked(&state, &world_name, false).await
}

async fn set_world_blocked(
    state: &SharedState,
    world_name: &str,
    blocked: bool,
) -> Result<Json<Value>, ApiError> {
    let world = parse_world(world_name)?;
    if !store::admin_set_world_blocked(&state.pool, &world, blocked).await? {
        return Err(ApiError::not_found(format!(
            "World \"{world_name}\" was not found."
        )));
    }
    Ok(Json(
        json!({ "ok": true, "world": world_name, "blocked": blocked }),
    ))
}

/// GET /admin/blocked
pub async fn list_blocked(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    admin(&state, &headers)?;
    let rows = store::admin_list_blocked(&state.pool).await?;
    let blocked: Vec<Value> = rows
        .into_iter()
        .map(|b| json!({ "wallet": b.wallet, "createdAt": rfc3339(b.created_at), "updatedAt": rfc3339(b.updated_at) }))
        .collect();
    Ok(Json(json!({ "blocked": blocked })))
}

fn parse_wallet(raw: &str) -> Result<Address, ApiError> {
    Address::parse(raw).map_err(|_| ApiError::bad_request(format!("Invalid wallet: {raw}.")))
}

/// POST /admin/blocked/{wallet}
pub async fn block_wallet(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(wallet): Path<String>,
) -> Result<Json<Value>, ApiError> {
    admin(&state, &headers)?;
    let wallet = parse_wallet(&wallet)?;
    store::admin_block_wallet(&state.pool, &wallet).await?;
    state
        .revoked(super::Revocation::Platform {
            address: wallet.clone(),
        })
        .await;
    Ok(Json(
        json!({ "ok": true, "wallet": wallet.as_str(), "blocked": true }),
    ))
}

/// DELETE /admin/blocked/{wallet}
pub async fn unblock_wallet(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(wallet): Path<String>,
) -> Result<Json<Value>, ApiError> {
    admin(&state, &headers)?;
    let wallet = parse_wallet(&wallet)?;
    if !store::admin_unblock_wallet(&state.pool, &wallet).await? {
        return Err(ApiError::not_found(format!(
            "Wallet \"{}\" is not on the block list.",
            wallet.as_str()
        )));
    }
    Ok(Json(
        json!({ "ok": true, "wallet": wallet.as_str(), "blocked": false }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct AccessLogQuery {
    #[serde(default)]
    pub world: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// GET /admin/access-log
pub async fn access_log(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<AccessLogQuery>,
) -> Result<Json<Value>, ApiError> {
    admin(&state, &headers)?;
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = q.offset.unwrap_or(0).max(0);
    let world = q
        .world
        .as_deref()
        .filter(|w| !w.is_empty())
        .map(parse_world)
        .transpose()?;
    let address = q
        .address
        .as_deref()
        .filter(|a| !a.is_empty())
        .map(parse_wallet)
        .transpose()?;
    let rows =
        store::admin_query_access_log(&state.pool, world.as_ref(), address.as_ref(), limit, offset)
            .await?;
    let entries: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "worldName": r.world_name,
                "address": r.address,
                "action": r.action,
                "room": r.room,
                "createdAt": rfc3339(r.created_at),
            })
        })
        .collect();
    Ok(Json(
        json!({ "limit": limit, "offset": offset, "entries": entries }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct BanStatusQuery {
    pub address: String,
    #[serde(default)]
    pub parcel: Option<String>,
}

/// GET /admin/worlds/{world_name}/ban-status: the platform ban and, with a
/// parcel, the ban from the scene standing on it, both from the shared
/// tables through `Policy::is_banned` (the old route asked the gatekeeper
/// over HTTP and answered 503 without one).
pub async fn world_ban_status(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(world_name): Path<String>,
    Query(q): Query<BanStatusQuery>,
) -> Result<Json<Value>, ApiError> {
    admin(&state, &headers)?;
    let world = parse_world(&world_name)?;
    let address = parse_wallet(&q.address)?;
    let now = chrono::Utc::now().naive_utc();
    let scene: Option<SceneId> = match q.parcel.as_deref() {
        Some(raw) => {
            let parcel = Parcel::parse(raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
            rooms::world_scene_at(&state.pool, &world, parcel)
                .await
                .map_err(|e| ApiError::internal(format!("resolving the scene at {parcel}: {e}")))?
                .map(|d| d.scene_id)
        }
        None => None,
    };
    let platform = state.policy.is_banned(&address, None, None, now).await?;
    let platform_banned = matches!(platform, Some(Ban::Blocked | Ban::Platform));
    let scene_banned = match &scene {
        Some(scene) => Some(
            state
                .policy
                .is_scene_banned(scene.as_str(), &address)
                .await?,
        ),
        None => q.parcel.as_ref().map(|_| false),
    };
    Ok(Json(json!({
        "world": world_name,
        "address": address.as_str(),
        "platformBanned": platform_banned,
        "parcel": q.parcel,
        "sceneBanned": scene_banned,
    })))
}
