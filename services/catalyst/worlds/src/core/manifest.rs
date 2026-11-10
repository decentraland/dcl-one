//! `GET /world/{world_name}/manifest` (the occupied parcels and the spawn of
//! a world, for the creator hub) and `GET /worlds` (the public world listing
//! with its filters and ordering).

use axum::Json;
use axum::extract::{Path, Query, State};
use serde_json::{Value, json};

use super::SharedState;
use super::http::{ApiError, QueryPairs, authorized_deployer, pagination, query_param};
use super::permissions::parse_world;
use super::settings_policy::text_len;
use super::store::{
    self, OrderDirection, WorldInfoRow, WorldsListFilters, WorldsListOptions, WorldsOrderBy,
};

pub async fn get_world_manifest(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let world = parse_world(&world_name)?;
    if !state
        .names
        .check_name_deny_list(&state.pool, &world_name)
        .await
    {
        return Err(ApiError::not_found(format!(
            "World \"{world_name}\" not found."
        )));
    }
    let manifest = store::world_manifest(&state.pool, &world)
        .await?
        .ok_or_else(|| {
            ApiError::not_found(format!("World \"{world_name}\" has no scenes deployed."))
        })?;
    let spawn = manifest.spawn_coordinates.as_deref().unwrap_or("0,0");
    let mut parts = spawn.split(',');
    let x = parts.next().unwrap_or("0");
    let y = parts.next().unwrap_or("0");
    Ok(Json(json!({
        "occupied": manifest.parcels,
        "spawn_coordinate": { "x": x, "y": y },
        "total": manifest.total,
    })))
}

const MAX_SEARCH_TERM_LENGTH: usize = 64;

/// GET /worlds. Each parameter is read as `URLSearchParams.get` reads it
/// (the first of a repeated key); deny-listed bare names are left out.
pub async fn get_worlds(
    State(state): State<SharedState>,
    Query(q): Query<QueryPairs>,
) -> Result<Json<Value>, ApiError> {
    let (limit, offset) = pagination(&q);

    let deployer = authorized_deployer(&q)?;
    let search = match query_param(&q, "search")
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(s) if text_len(s) > MAX_SEARCH_TERM_LENGTH => {
            return Err(ApiError::bad_request(format!(
                "Invalid search parameter: must be at most {MAX_SEARCH_TERM_LENGTH} characters."
            )));
        }
        Some(s) => Some(s.to_string()),
        None => None,
    };
    let has_deployed_scenes = query_param(&q, "has_deployed_scenes").map(|v| v == "true");
    let order_by = match query_param(&q, "sort").unwrap_or("name") {
        "name" => WorldsOrderBy::Name,
        "last_deployed_at" => WorldsOrderBy::LastDeployedAt,
        other => {
            return Err(ApiError::bad_request(format!(
                "Invalid sort parameter: {other}. Valid values are: name, last_deployed_at"
            )));
        }
    };
    let order_direction = match query_param(&q, "order").unwrap_or("asc") {
        "asc" => OrderDirection::Asc,
        "desc" => OrderDirection::Desc,
        other => {
            return Err(ApiError::bad_request(format!(
                "Invalid order parameter: {other}. Valid values are: asc, desc"
            )));
        }
    };

    let filters = WorldsListFilters {
        authorized_deployer: deployer,
        search,
        has_deployed_scenes,
        banned_names: state.names.snapshot(&state.pool).await.banned_names(),
    };
    let options = WorldsListOptions {
        limit,
        offset,
        order_by,
        order_direction,
    };
    let (worlds, total) = store::list_worlds_public(&state.pool, &filters, &options).await?;
    let worlds: Vec<Value> = worlds.iter().map(world_info_json).collect();
    Ok(Json(json!({ "worlds": worlds, "total": total })))
}

fn rfc3339(t: chrono::NaiveDateTime) -> String {
    t.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn world_info_json(w: &WorldInfoRow) -> Value {
    let shape = match (w.min_x, w.max_x, w.min_y, w.max_y) {
        (Some(x1), Some(x2), Some(y1), Some(y2)) => {
            json!({ "x1": x1, "x2": x2, "y1": y1, "y2": y2 })
        }
        _ => Value::Null,
    };
    json!({
        "name": w.name,
        "owner": w.owner,
        "title": w.title,
        "description": w.description,
        "shape": shape,
        "content_rating": w.content_rating,
        "spawn_coordinates": w.spawn_coordinates,
        "skybox_time": w.skybox_time,
        "categories": w.categories,
        "single_player": w.single_player,
        "show_in_places": w.show_in_places,
        "thumbnail_hash": w.thumbnail_hash,
        "last_deployed_at": w.last_deployed_at.map(rfc3339),
        "blocked_since": w.blocked_since.map(rfc3339),
        "deployed_scenes": w.deployed_scenes,
    })
}
