use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

use super::SharedState;
use super::about_comms::{AboutComms, build_comms_config};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutResponse {
    pub healthy: bool,
    pub content: AboutContent,
    pub lambdas: AboutLambdas,
    pub configurations: AboutConfigurations,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comms: Option<AboutComms>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bff: Option<AboutBff>,
    pub accepting_users: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutContent {
    pub healthy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub synchronization_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    pub public_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutLambdas {
    pub healthy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    pub public_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutConfigurations {
    pub network_id: u64,
    pub global_scenes_urn: Vec<String>,
    pub scenes_urn: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm_name: Option<String>,
    pub map: AboutConfigurationsMap,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutConfigurationsMap {
    pub minimap_enabled: bool,
    pub sizes: Vec<AboutMapSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub satellite_view: Option<AboutMapView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parcel_view: Option<AboutParcelView>,
}

#[derive(Serialize)]
pub struct AboutMapSize {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutMapView {
    pub version: String,
    pub base_url: String,
    pub suffix_url: String,
    pub top_left_offset: AboutMapOffset,
}

#[derive(Serialize)]
pub struct AboutMapOffset {
    pub x: i32,
    pub y: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutParcelView {
    pub version: String,
    pub image_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutBff {
    pub healthy: bool,
    pub protocol_version: String,
    pub user_count: u64,
    pub public_url: String,
}

fn network_id(eth_network: &str) -> u64 {
    match eth_network {
        "mainnet" => 1,
        "sepolia" => 11155111,
        "goerli" => 5,
        "ropsten" => 3,
        "rinkeby" => 4,
        "kovan" => 42,
        _ => 1,
    }
}

fn content_is_healthy(sync_state: &str) -> bool {
    sync_state == "Syncing"
}

/// The genesis-city parcel bounds every realm advertises.
fn map_sizes() -> Vec<AboutMapSize> {
    vec![
        AboutMapSize {
            left: -150,
            top: 150,
            right: 150,
            bottom: -150,
        },
        AboutMapSize {
            left: 62,
            top: 158,
            right: 162,
            bottom: 151,
        },
        AboutMapSize {
            left: 151,
            top: 150,
            right: 163,
            bottom: 59,
        },
    ]
}

/// Every realm advertises the map block (the old server never omitted it,
/// and the about-golden fixture requires it); a view is left out when this
/// node has none to point at: no `MAP_SATELLITE_BASE_URL` (its own pyramid
/// is not the ADR-250 scheme), or its own minimap without a tile grid.
fn map_configuration(
    settings: &super::Settings,
    parcel_view: Option<&str>,
) -> AboutConfigurationsMap {
    AboutConfigurationsMap {
        minimap_enabled: true,
        sizes: map_sizes(),
        satellite_view: settings
            .map_satellite_base_url
            .clone()
            .map(|base_url| AboutMapView {
                version: "v1".to_string(),
                base_url,
                suffix_url: ".jpg".to_string(),
                top_left_offset: AboutMapOffset { x: -2, y: -6 },
            }),
        parcel_view: parcel_view.map(|image_url| AboutParcelView {
            version: "v1".to_string(),
            image_url: image_url.to_string(),
        }),
    }
}

pub async fn get_about(State(state): State<SharedState>) -> impl IntoResponse {
    let sync_state = state.sync.state_name().to_string();
    let content_healthy = content_is_healthy(&sync_state);
    let settings = &state.settings;

    let mut healthy = content_healthy;
    let mut accepting_users = healthy;
    let comms = state.about_comms().map(|about| {
        let probe = about.probe();
        let comms = build_comms_config(probe, &about.settings, about.sfu.is_alive());
        healthy = healthy && comms.healthy;
        accepting_users = accepting_users && about.under_capacity(probe);
        comms
    });

    let body = AboutResponse {
        healthy,
        content: AboutContent {
            healthy: content_healthy,
            version: Some(settings.version.clone()),
            synchronization_status: sync_state,
            commit_hash: settings.commit_hash.clone(),
            public_url: settings.content_public_url.clone(),
        },
        lambdas: AboutLambdas {
            healthy: true,
            version: Some(settings.version.clone()),
            commit_hash: settings.commit_hash.clone(),
            public_url: settings.lambdas_public_url.clone(),
        },
        configurations: AboutConfigurations {
            network_id: network_id(&settings.eth_network),
            global_scenes_urn: vec![],
            scenes_urn: vec![],
            realm_name: settings.realm_name.clone(),
            map: map_configuration(settings, state.parcel_view_url()),
        },
        comms,
        bff: Some(AboutBff {
            healthy: true,
            protocol_version: "1.0_0".to_string(),
            user_count: 0,
            public_url: "/bff".to_string(),
        }),
        // The console's accepting-users toggle takes a healthy node out of
        // the realm pickers; the old live node stored the toggle without
        // reading it here.
        accepting_users: accepting_users && state.is_accepting_users(),
    };

    let status = if healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status, Json(body))
}

/// Upstream's lambdas default when the node sets no `COMMS_MAX_USERS`.
const DEFAULT_MAX_USERS: u64 = 100;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RealmInfo {
    pub server_name: String,
    pub url: String,
    pub users_count: u64,
    pub max_users: u64,
    pub user_parcels: Vec<[i32; 2]>,
}

fn node_base_url(content_public_url: &str) -> String {
    let trimmed = content_public_url.trim_end_matches('/');
    trimmed
        .strip_suffix("/content")
        .unwrap_or(trimmed)
        .to_string()
}

/// `GET /lambdas/explore/realms`: this node as the one realm it knows, with
/// the comms core's live peers (one `userParcels` entry per peer).
pub async fn get_explore_realms(State(state): State<SharedState>) -> Json<Vec<RealmInfo>> {
    let settings = &state.settings;
    let url = node_base_url(&settings.content_public_url);
    let (users_count, max_users, mut user_parcels) = match state.about_comms() {
        Some(about) => {
            let mut parcels = Vec::new();
            for ((x, y), peers) in about.parcel_counts() {
                parcels.extend(std::iter::repeat_n([x, y], peers as usize));
            }
            (
                about.probe().user_count,
                about.settings.max_users.unwrap_or(DEFAULT_MAX_USERS),
                parcels,
            )
        }
        None => (0, DEFAULT_MAX_USERS, Vec::new()),
    };
    user_parcels.sort_unstable();
    Json(vec![RealmInfo {
        server_name: settings.realm_name.clone().unwrap_or_else(|| url.clone()),
        url,
        users_count,
        max_users,
        user_parcels,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_syncing_is_healthy() {
        assert!(content_is_healthy("Syncing"));
        assert!(!content_is_healthy("Bootstrapping"));
        assert!(!content_is_healthy("Partially synced"));
        assert!(!content_is_healthy("Failed"));
    }

    #[test]
    fn realm_url_is_the_node_root() {
        assert_eq!(
            node_base_url("http://127.0.0.1:5210/content/"),
            "http://127.0.0.1:5210"
        );
        assert_eq!(
            node_base_url("https://peer.example/content"),
            "https://peer.example"
        );
        assert_eq!(
            node_base_url("https://peer.example/"),
            "https://peer.example"
        );
    }

    #[test]
    fn network_ids() {
        assert_eq!(network_id("mainnet"), 1);
        assert_eq!(network_id("sepolia"), 11155111);
        assert_eq!(network_id("other"), 1);
    }

    #[test]
    fn map_carries_both_views() {
        let mut settings = super::super::Settings::for_tests();
        settings.map_parcel_view_url = "http://map/minimap.png".into();
        let map = map_configuration(&settings, settings.parcel_view_url(false));
        assert_eq!(
            map.satellite_view.unwrap().base_url,
            "http://127.0.0.1:5162/satellite"
        );
        assert_eq!(map.parcel_view.unwrap().image_url, "http://map/minimap.png");
        assert_eq!(map.sizes.len(), 3);
    }

    /// No satellite base, no satellite view (the node's own pyramid is not
    /// the ADR-250 scheme); an island's own minimap only once it has a grid.
    #[test]
    fn map_leaves_out_the_views_this_node_cannot_serve() {
        let mut settings = super::super::Settings::for_tests();
        settings.map_satellite_base_url = None;
        settings.map_parcel_view_needs_grid = true;
        let map = map_configuration(&settings, settings.parcel_view_url(false));
        assert!(map.satellite_view.is_none() && map.parcel_view.is_none());
        let json = serde_json::to_value(&map).unwrap();
        assert!(
            json.get("satelliteView").is_none() && json.get("parcelView").is_none(),
            "{json}"
        );
        assert_eq!(json["minimapEnabled"], true);
        let map = map_configuration(&settings, settings.parcel_view_url(true));
        assert_eq!(
            map.parcel_view.unwrap().image_url,
            "http://127.0.0.1:5162/v1/minimap.png"
        );
    }
}
