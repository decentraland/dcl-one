use axum::Json;
use axum::extract::{Path, State};
use chrono::SecondsFormat;
use serde::Serialize;
use serde_json::Value;

use super::SharedState;
use super::queries;
use crate::content::errors::AppResult;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct WorldSceneEntry {
    pub world_name: String,
    pub deployer: String,
    #[cfg_attr(feature = "ts", ts(type = "unknown[]"))]
    pub deployment_auth_chain: Value,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub entity: Value,
    pub entity_id: String,
    pub parcels: Vec<String>,
    pub size: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct SceneListResponse {
    pub scenes: Vec<WorldSceneEntry>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
}

/// GET /world/{world_name}/scenes: every scene row of the world, by entity id.
pub async fn list_scenes(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
) -> AppResult<Json<SceneListResponse>> {
    let mut rows = queries::list_scenes_full(&state.pool, &world_name).await?;
    if rows.is_empty() && state.fill_world(&world_name).await {
        rows = queries::list_scenes_full(&state.pool, &world_name).await?;
    }
    let scenes: Vec<WorldSceneEntry> = rows
        .into_iter()
        .map(|r| WorldSceneEntry {
            world_name: r.world_name,
            deployer: r.deployer,
            deployment_auth_chain: r.deployment_auth_chain,
            entity: r.entity,
            entity_id: r.entity_id,
            parcels: r.parcels,
            size: r.size.to_string(),
            status: "DEPLOYED".to_string(),
            created_at: r
                .created_at
                .and_utc()
                .to_rfc3339_opts(SecondsFormat::Millis, true),
            updated_at: r
                .updated_at
                .and_utc()
                .to_rfc3339_opts(SecondsFormat::Millis, true),
        })
        .collect();
    let total = scenes.len() as i64;
    Ok(Json(SceneListResponse { scenes, total }))
}
