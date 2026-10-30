//! realm-provider: `GET /main/about` (also `/main` and `/about`), the realm
//! doc unity-explorer boots Genesis City from; `GET /realms`; `GET /status`.
//! Anything else answers upstream's `404 Not found` (`/main/preview-wearables`
//! in the boot corpus). The content block reads the node's sync status
//! directly: `healthy`/`acceptingUsers` require `Syncing` content and, when
//! attached, a healthy comms core. Discovery counts use that same core probe.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use super::Shared;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutResponse {
    pub healthy: bool,
    pub content: AboutContent,
    pub lambdas: AboutLambdas,
    pub configurations: AboutConfigurations,
    pub bff: AboutBff,
    pub accepting_users: bool,
    pub comms: AboutComms,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutContent {
    pub healthy: bool,
    pub version: String,
    pub synchronization_status: String,
    pub commit_hash: String,
    pub public_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutLambdas {
    pub healthy: bool,
    pub version: String,
    pub commit_hash: String,
    pub public_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutConfigurations {
    pub network_id: u64,
    pub global_scenes_urn: Vec<String>,
    pub scenes_urn: Vec<String>,
    pub realm_name: String,
    pub map: MapConfig,
    pub local_scene_parcels: Vec<String>,
    pub skybox: Skybox,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapConfig {
    pub minimap_enabled: bool,
    pub sizes: Vec<MapSize>,
    /// Left out without `MAP_SATELLITE_BASE_URL` (see the content /about).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub satellite_view: Option<SatelliteView>,
    /// Left out while an island's own minimap has no tile grid to draw.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parcel_view: Option<ParcelView>,
}

#[derive(Debug, Serialize)]
pub struct MapSize {
    pub left: i64,
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SatelliteView {
    pub version: String,
    pub base_url: String,
    pub suffix_url: String,
    pub top_left_offset: Offset,
}

#[derive(Debug, Serialize)]
pub struct Offset {
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParcelView {
    pub version: String,
    pub image_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skybox {
    pub fixed_hour: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutBff {
    pub healthy: bool,
    pub protocol_version: String,
    pub user_count: u64,
    pub public_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutComms {
    pub version: String,
    pub commit_hash: String,
    pub healthy: bool,
    pub protocol: String,
    pub users_count: u64,
    pub adapter: String,
    pub fixed_adapter: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RealmEntry {
    pub server_name: String,
    pub url: String,
    pub users_count: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub version: String,
    pub current_time: i64,
    pub commit_hash: String,
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/main", get(main_about))
        .route("/main/about", get(main_about))
        .route("/about", get(main_about))
        .route("/realms", get(realms))
        .route("/status", get(status))
        .fallback(not_found)
}

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

/// The genesis-city parcel bounds every realm advertises.
fn map_sizes() -> Vec<MapSize> {
    vec![
        MapSize {
            left: -150,
            top: 150,
            right: 150,
            bottom: -150,
        },
        MapSize {
            left: 62,
            top: 158,
            right: 162,
            bottom: 151,
        },
        MapSize {
            left: 151,
            top: 150,
            right: 163,
            bottom: 59,
        },
    ]
}

pub fn about(state: &Shared) -> AboutResponse {
    let cfg = &state.settings;
    let content = &state.content.settings;
    let sync_state = state.content.sync.state_name().to_string();
    let content_healthy = sync_state == "Syncing";
    let probe = state.content.about_comms().map(|about| about.probe());
    let comms_healthy = probe.is_none_or(|probe| probe.healthy);
    let users_count = probe.map_or(0, |probe| probe.user_count);
    let healthy = content_healthy && comms_healthy;
    let commit_hash = content.commit_hash.clone().unwrap_or_default();
    AboutResponse {
        healthy,
        content: AboutContent {
            healthy: content_healthy,
            version: content.version.clone(),
            synchronization_status: sync_state,
            commit_hash: commit_hash.clone(),
            public_url: content.content_public_url.clone(),
        },
        lambdas: AboutLambdas {
            healthy: true,
            version: content.version.clone(),
            commit_hash: commit_hash.clone(),
            public_url: content.lambdas_public_url.clone(),
        },
        configurations: AboutConfigurations {
            network_id: cfg.network_id,
            global_scenes_urn: Vec::new(),
            scenes_urn: Vec::new(),
            realm_name: cfg.realm_name.clone(),
            map: MapConfig {
                minimap_enabled: true,
                sizes: map_sizes(),
                satellite_view: content.map_satellite_base_url.clone().map(|base_url| {
                    SatelliteView {
                        version: "v1".to_string(),
                        base_url,
                        suffix_url: ".jpg".to_string(),
                        top_left_offset: Offset { x: -2, y: -6 },
                    }
                }),
                parcel_view: content
                    .parcel_view_url(state.map.is_ready())
                    .map(|image_url| ParcelView {
                        version: "v1".to_string(),
                        image_url: image_url.to_string(),
                    }),
            },
            local_scene_parcels: Vec::new(),
            skybox: Skybox { fixed_hour: -1 },
        },
        bff: AboutBff {
            healthy: true,
            protocol_version: "1.0_0".to_string(),
            user_count: users_count,
            public_url: cfg.bff_url.clone(),
        },
        accepting_users: healthy,
        comms: AboutComms {
            version: cfg.version.clone(),
            commit_hash,
            healthy: comms_healthy,
            protocol: "v3".to_string(),
            users_count,
            adapter: cfg.comms_adapter.clone(),
            fixed_adapter: cfg.comms_fixed_adapter.clone(),
        },
    }
}

async fn main_about(State(state): State<Shared>) -> Json<AboutResponse> {
    Json(about(&state))
}

async fn realms(State(state): State<Shared>) -> Json<Vec<RealmEntry>> {
    let probe = state.content.about_comms().map(|about| about.probe());
    Json(if probe.is_none_or(|probe| probe.healthy) {
        vec![RealmEntry {
            server_name: state.settings.realm_name.clone(),
            url: state.settings.public_url.clone(),
            users_count: probe.map_or(0, |probe| probe.user_count),
        }]
    } else {
        Vec::new()
    })
}

async fn status(State(state): State<Shared>) -> Json<StatusResponse> {
    Json(StatusResponse {
        version: state.settings.version.clone(),
        current_time: chrono::Utc::now().timestamp_millis(),
        commit_hash: state.settings.commit_hash.clone(),
    })
}
