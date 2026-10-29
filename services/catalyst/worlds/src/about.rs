use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;
use serde_json::Value;

use super::SharedState;
use super::queries::{self, WorldScene};
use crate::content::errors::{AppError, AppResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct MinimapConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub data_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub estate_image: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct SkyboxConfig {
    pub fixed_hour: Option<f64>,
    pub textures: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct AboutMap {
    pub minimap_enabled: bool,
    #[cfg_attr(feature = "ts", ts(type = "unknown[]"))]
    pub sizes: Vec<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct AboutConfigurations {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub network_id: i64,
    pub global_scenes_urn: Vec<String>,
    pub scenes_urn: Vec<String>,
    pub minimap: MinimapConfig,
    pub skybox: SkyboxConfig,
    pub realm_name: String,
    pub map: AboutMap,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct AboutContentStatus {
    pub synchronization_status: String,
    pub healthy: bool,
    pub public_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct AboutServiceStatus {
    pub healthy: bool,
    pub public_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct AboutComms {
    pub healthy: bool,
    pub protocol: String,
    pub adapter: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct AboutResponse {
    pub healthy: bool,
    pub accepting_users: bool,
    pub spawn_coordinates: Option<String>,
    pub configurations: AboutConfigurations,
    pub content: AboutContentStatus,
    pub lambdas: AboutServiceStatus,
    pub comms: AboutComms,
}

/// GET /world/{world_name}/about.
pub async fn get_about(
    State(state): State<SharedState>,
    Path(world_name): Path<String>,
) -> AppResult<Json<AboutResponse>> {
    let mut about = queries::world_about(&state.pool, &world_name).await?;
    if about.scenes.is_empty() && state.fill_world(&world_name).await {
        about = queries::world_about(&state.pool, &world_name).await?;
    }
    Ok(Json(build_about(&state.settings, &world_name, about)?))
}

pub fn build_about(
    cfg: &super::Settings,
    world_name: &str,
    about: queries::WorldAbout,
) -> AppResult<AboutResponse> {
    let world = &about.world;
    let scenes = &about.scenes;

    if scenes.is_empty() {
        return Err(AppError::not_found(format!(
            "World \"{world_name}\" has no scenes deployed."
        )));
    }
    if let Some(since) = world.as_ref().and_then(|w| w.blocked_since) {
        return Err(AppError::Unauthorized(format!(
            "World \"{world_name}\" has been blocked since {} as it exceeded its allowed storage space.",
            since
                .and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        )));
    }

    let base_url = &cfg.public_url;
    let scenes_urn: Vec<String> = scenes
        .iter()
        .map(|s| {
            format!(
                "urn:decentraland:entity:{}?=&baseUrl={base_url}/contents/",
                s.entity_id
            )
        })
        .collect();

    let primary = &scenes[0];
    let mut rt = RuntimeMeta::from_scene(world_name, primary);
    rt.name = resolve_realm_name(
        &rt.name,
        world
            .as_ref()
            .and_then(|w| w.realm_name_override.as_deref()),
        cfg.realm_name_strip_ens,
        is_locally_published(scenes),
    );
    if let Some(w) = world {
        if let Some(t) = w.skybox_time {
            rt.skybox_fixed_time = Some(t as f64);
        }
        if w.single_player {
            rt.fixed_adapter = Some("offline:offline".to_string());
        }
    }

    let spawn_coordinates = world
        .as_ref()
        .and_then(|w| w.spawn_coordinates.clone())
        .or_else(|| {
            primary.entity["metadata"]["scene"]["base"]
                .as_str()
                .map(str::to_string)
        });

    let url_for_file = |filename: &Option<String>, default_image: &str| -> String {
        match filename {
            Some(f) => format!("{base_url}/contents/{f}"),
            None => default_image.to_string(),
        }
    };
    let data_image = (rt.minimap_visible || rt.minimap_data_image.is_some())
        .then(|| url_for_file(&rt.minimap_data_image, &cfg.map_parcel_view_url));
    let estate_image = (rt.minimap_visible || rt.minimap_estate_image.is_some())
        .then(|| url_for_file(&rt.minimap_estate_image, &cfg.map_estate_view_url));

    let adapter = resolve_fixed_adapter(
        world_name,
        rt.fixed_adapter.as_deref(),
        base_url,
        cfg.comms_online && cfg.sfu.is_alive(),
    );

    Ok(AboutResponse {
        healthy: true,
        accepting_users: true,
        spawn_coordinates,
        configurations: AboutConfigurations {
            network_id: cfg.network_id,
            global_scenes_urn: cfg.global_scenes_urn.clone(),
            scenes_urn,
            minimap: MinimapConfig {
                enabled: rt.minimap_visible,
                data_image,
                estate_image,
            },
            skybox: SkyboxConfig {
                fixed_hour: rt.skybox_fixed_time,
                textures: rt
                    .skybox_textures
                    .iter()
                    .map(|t| format!("{base_url}/contents/{t}"))
                    .collect(),
            },
            realm_name: rt.name,
            map: AboutMap {
                minimap_enabled: false,
                sizes: Vec::new(),
            },
        },
        content: AboutContentStatus {
            synchronization_status: "Syncing".to_string(),
            healthy: true,
            public_url: cfg.content_public_url.clone(),
        },
        lambdas: AboutServiceStatus {
            healthy: true,
            public_url: cfg.lambdas_public_url.clone(),
        },
        comms: AboutComms {
            healthy: true,
            protocol: "v3".to_string(),
            adapter,
        },
    })
}

const ENS_WORLD_SUFFIX: &str = ".dcl.eth";

/// Whether an owner deployed to this node, as opposed to the mirror copying
/// someone else's world here. A mirror keeps the ENS realm name pointing
/// clients at the original registry; a local publish is authoritative and
/// must not lose to it.
fn is_locally_published(scenes: &[WorldScene]) -> bool {
    scenes.iter().any(WorldScene::is_local)
}

fn strip_ens_suffix(name: &str) -> &str {
    let cut = name.len().saturating_sub(ENS_WORLD_SUFFIX.len());
    match name[cut..].eq_ignore_ascii_case(ENS_WORLD_SUFFIX) && cut > 0 {
        true => &name[..cut],
        false => name,
    }
}

/// An explicit override always wins: it is the only value here a republish
/// cannot overwrite, which is why the column exists.
fn resolve_realm_name(
    derived: &str,
    override_name: Option<&str>,
    strip_ens: bool,
    locally_published: bool,
) -> String {
    if let Some(name) = override_name.map(str::trim).filter(|n| !n.is_empty()) {
        return name.to_string();
    }
    match strip_ens && locally_published {
        true => strip_ens_suffix(derived).to_string(),
        false => derived.to_string(),
    }
}

fn resolve_fixed_adapter(
    world_name: &str,
    fixed_adapter: Option<&str>,
    base_url: &str,
    comms_online: bool,
) -> String {
    if fixed_adapter == Some("offline:offline") || !comms_online {
        return "fixed-adapter:offline:offline".to_string();
    }
    format!(
        "fixed-adapter:signed-login:{base_url}/worlds/{}/comms",
        world_name.to_lowercase()
    )
}

struct RuntimeMeta {
    name: String,
    minimap_visible: bool,
    minimap_data_image: Option<String>,
    minimap_estate_image: Option<String>,
    skybox_fixed_time: Option<f64>,
    skybox_textures: Vec<String>,
    fixed_adapter: Option<String>,
}

impl RuntimeMeta {
    fn from_scene(world_name: &str, scene: &WorldScene) -> Self {
        let wc = scene
            .entity
            .get("metadata")
            .and_then(|m| m.get("worldConfiguration"));
        let name = wc
            .and_then(|c| c.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| world_name.to_string());
        let mini_map = wc.and_then(|c| c.get("miniMapConfig"));
        let minimap_visible = mini_map
            .and_then(|m| m.get("visible"))
            .and_then(Value::as_bool)
            .or_else(|| {
                wc.and_then(|c| c.get("minimapVisible"))
                    .and_then(Value::as_bool)
            })
            .unwrap_or(false);
        let skybox = wc.and_then(|c| c.get("skyboxConfig"));
        let skybox_fixed_time = skybox
            .and_then(|s| s.get("fixedTime"))
            .and_then(Value::as_f64);

        let content = scene.entity.get("content").and_then(Value::as_array);
        let resolve = |filename: &str| -> Option<String> {
            content.and_then(|arr| {
                arr.iter()
                    .find(|c| c.get("file").and_then(Value::as_str) == Some(filename))
                    .and_then(|c| c.get("hash").and_then(Value::as_str))
                    .map(str::to_string)
            })
        };
        let minimap_data_image = mini_map
            .and_then(|m| m.get("dataImage"))
            .and_then(Value::as_str)
            .and_then(&resolve);
        let minimap_estate_image = mini_map
            .and_then(|m| m.get("estateImage"))
            .and_then(Value::as_str)
            .and_then(&resolve);
        let skybox_textures = skybox
            .and_then(|s| s.get("textures"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .filter_map(resolve)
                    .collect()
            })
            .unwrap_or_default();
        let fixed_adapter = wc
            .and_then(|c| c.get("fixedAdapter"))
            .and_then(Value::as_str)
            .map(str::to_string);

        RuntimeMeta {
            name,
            minimap_visible,
            minimap_data_image,
            minimap_estate_image,
            skybox_fixed_time,
            skybox_textures,
            fixed_adapter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ZERO_ADDRESS;
    use crate::queries::{WorldAbout, WorldRecord};
    use serde_json::json;

    fn scene(mirror_source: Option<&str>) -> WorldScene {
        WorldScene {
            entity_id: "bafy".into(),
            entity: json!({}),
            parcels: vec!["0,0".into()],
            deployer: if mirror_source.is_some() {
                ZERO_ADDRESS.into()
            } else {
                "0xabc".into()
            },
            mirror_source: mirror_source.map(str::to_string),
        }
    }

    #[test]
    fn locally_published_worlds_lose_the_ens_suffix() {
        assert_eq!(
            resolve_realm_name("swissverse.dcl.eth", None, true, true),
            "swissverse"
        );
    }

    #[test]
    fn mirrored_worlds_keep_the_ens_suffix() {
        assert_eq!(
            resolve_realm_name("swissverse.dcl.eth", None, true, false),
            "swissverse.dcl.eth"
        );
    }

    #[test]
    fn the_override_wins_over_every_policy() {
        assert_eq!(
            resolve_realm_name("swissverse.dcl.eth", Some(" swiss-cube "), false, false),
            "swiss-cube"
        );
        assert_eq!(
            resolve_realm_name("swissverse.dcl.eth", Some("keep.dcl.eth"), true, true),
            "keep.dcl.eth"
        );
    }

    #[test]
    fn a_blank_override_is_not_an_override() {
        assert_eq!(
            resolve_realm_name("plain-name", Some("   "), false, false),
            "plain-name"
        );
    }

    #[test]
    fn stripping_leaves_non_ens_and_bare_suffix_names_alone() {
        assert_eq!(
            resolve_realm_name("swiss-cube", None, true, true),
            "swiss-cube"
        );
        assert_eq!(resolve_realm_name(".dcl.eth", None, true, true), ".dcl.eth");
    }

    #[test]
    fn only_a_scene_without_a_mirror_source_counts_as_locally_published() {
        assert!(!is_locally_published(&[scene(Some("https://up.example"))]));
        assert!(is_locally_published(&[
            scene(Some("https://up.example")),
            scene(None)
        ]));
    }

    #[test]
    fn single_player_override_maps_to_offline_adapter() {
        assert_eq!(
            resolve_fixed_adapter(
                "foo",
                Some("offline:offline"),
                "https://worlds.example",
                true
            ),
            "fixed-adapter:offline:offline"
        );
    }

    #[test]
    fn no_override_falls_back_to_signed_login() {
        assert_eq!(
            resolve_fixed_adapter("Foo", None, "https://worlds.example", true),
            "fixed-adapter:signed-login:https://worlds.example/worlds/foo/comms"
        );
    }

    #[test]
    fn an_unreachable_sfu_serves_the_offline_adapter() {
        assert_eq!(
            resolve_fixed_adapter("foo", None, "https://worlds.example", false),
            "fixed-adapter:offline:offline"
        );
    }

    #[test]
    fn a_plain_http_node_still_advertises_signed_login() {
        assert_eq!(
            resolve_fixed_adapter("Foo", None, "http://127.0.0.1:5210", true),
            "fixed-adapter:signed-login:http://127.0.0.1:5210/worlds/foo/comms"
        );
    }

    #[test]
    fn a_public_url_under_the_worlds_prefix_reaches_the_nested_mint() {
        assert_eq!(
            resolve_fixed_adapter(
                "foo",
                None,
                "https://peer.example/worlds-content-server",
                true
            ),
            "fixed-adapter:signed-login:https://peer.example/worlds-content-server/worlds/foo/comms"
        );
    }

    fn entity(world_name: &str) -> Value {
        json!({
            "timestamp": 1000,
            "content": [
                {"file": "scene.json", "hash": "bafkreiscene"},
                {"file": "images/minimap.png", "hash": "bafkreiminimap"},
                {"file": "sky/1.png", "hash": "bafkreisky1"}
            ],
            "metadata": {
                "scene": {"base": "1,2", "parcels": ["1,2"]},
                "worldConfiguration": {
                    "name": world_name,
                    "miniMapConfig": {"visible": true, "dataImage": "images/minimap.png"},
                    "skyboxConfig": {"fixedTime": 36000, "textures": ["sky/1.png", "missing.png"]}
                }
            }
        })
    }

    #[test]
    fn about_points_scenes_and_files_at_this_node_and_keeps_mirrored_ens_names() {
        let cfg = super::super::Settings::for_tests("https://worlds.example.org/");
        let scenes = vec![WorldScene {
            entity_id: "bafkreientity".into(),
            entity: entity("mirrored.dcl.eth"),
            parcels: vec!["1,2".into()],
            deployer: ZERO_ADDRESS.into(),
            mirror_source: Some("https://upstream.example".into()),
        }];
        let about = build_about(
            &cfg,
            "mirrored.dcl.eth",
            WorldAbout {
                world: Some(WorldRecord {
                    name: "mirrored.dcl.eth".into(),
                    blocked_since: None,
                    spawn_coordinates: None,
                    skybox_time: Some(43200),
                    single_player: false,
                    realm_name_override: None,
                }),
                scenes,
            },
        )
        .unwrap();
        let v = serde_json::to_value(&about).unwrap();
        assert_eq!(
            v["configurations"]["scenesUrn"],
            json!([
                "urn:decentraland:entity:bafkreientity?=&baseUrl=https://worlds.example.org/contents/"
            ])
        );
        assert_eq!(v["configurations"]["realmName"], "mirrored.dcl.eth");
        assert_eq!(
            v["spawnCoordinates"], "1,2",
            "the primary scene's base when the row has none"
        );
        assert_eq!(
            v["configurations"]["skybox"]["fixedHour"], 43200.0,
            "the world row overrides the scene"
        );
        assert_eq!(
            v["configurations"]["skybox"]["textures"],
            json!(["https://worlds.example.org/contents/bafkreisky1"]),
            "a texture that names no content entry is dropped"
        );
        assert_eq!(v["configurations"]["minimap"]["enabled"], true);
        assert_eq!(
            v["configurations"]["minimap"]["dataImage"],
            "https://worlds.example.org/contents/bafkreiminimap"
        );
        assert_eq!(
            v["configurations"]["minimap"]["estateImage"], "http://127.0.0.1:5162/v1/estatemap.png",
            "a visible minimap without its own estate image falls back to the map service"
        );
        assert_eq!(
            v["comms"]["adapter"], "fixed-adapter:offline:offline",
            "a node without comms advertises the offline adapter"
        );
        assert_eq!(
            v["content"]["publicUrl"],
            "https://peer.example.org/content"
        );
        assert_eq!(v["healthy"], true);
        assert_eq!(v["acceptingUsers"], true);

        let online = super::super::Settings {
            comms_online: true,
            ..cfg.clone()
        };
        let local = build_about(
            &online,
            "Mine.dcl.eth",
            WorldAbout {
                world: None,
                scenes: vec![WorldScene {
                    entity_id: "bafkreientity".into(),
                    entity: entity("mine.dcl.eth"),
                    parcels: vec!["1,2".into()],
                    deployer: "0xabc".into(),
                    mirror_source: None,
                }],
            },
        )
        .unwrap();
        assert_eq!(local.configurations.realm_name, "mine");
        assert_eq!(
            local.comms.adapter,
            "fixed-adapter:signed-login:https://worlds.example.org/worlds/mine.dcl.eth/comms"
        );
    }

    #[test]
    fn a_scene_that_fixes_the_offline_adapter_stays_offline_with_comms_online() {
        let cfg = super::super::Settings {
            comms_online: true,
            ..super::super::Settings::for_tests("https://worlds.example.org")
        };
        let mut solo = entity("solo.dcl.eth");
        solo["metadata"]["worldConfiguration"]["fixedAdapter"] = json!("offline:offline");
        let about = build_about(
            &cfg,
            "solo.dcl.eth",
            WorldAbout {
                world: None,
                scenes: vec![WorldScene {
                    entity_id: "bafkreientity".into(),
                    entity: solo,
                    parcels: vec!["1,2".into()],
                    deployer: "0xabc".into(),
                    mirror_source: None,
                }],
            },
        )
        .unwrap();
        assert_eq!(about.comms.adapter, "fixed-adapter:offline:offline");
    }

    fn local_about(cfg: &super::super::Settings, world: Option<WorldRecord>) -> AboutResponse {
        build_about(
            cfg,
            "mine.dcl.eth",
            WorldAbout {
                world,
                scenes: vec![WorldScene {
                    entity_id: "bafkreientity".into(),
                    entity: entity("mine.dcl.eth"),
                    parcels: vec!["1,2".into()],
                    deployer: "0xabc".into(),
                    mirror_source: None,
                }],
            },
        )
        .unwrap()
    }

    #[test]
    fn a_single_player_world_stays_offline_with_comms_online() {
        let cfg = super::super::Settings {
            comms_online: true,
            ..super::super::Settings::for_tests("https://worlds.example.org")
        };
        let about = local_about(
            &cfg,
            Some(WorldRecord {
                name: "mine.dcl.eth".into(),
                blocked_since: None,
                spawn_coordinates: None,
                skybox_time: None,
                single_player: true,
                realm_name_override: None,
            }),
        );
        assert_eq!(about.comms.adapter, "fixed-adapter:offline:offline");
    }

    #[test]
    fn configured_comms_follow_the_shared_sfu_liveness() {
        let alive = super::super::Settings {
            comms_online: true,
            ..super::super::Settings::for_tests("https://worlds.example.org")
        };
        let down = super::super::Settings {
            sfu: crate::content::about_comms::SfuHealth::down_for_tests(),
            ..alive.clone()
        };
        assert_eq!(
            local_about(&alive, None).comms.adapter,
            "fixed-adapter:signed-login:https://worlds.example.org/worlds/mine.dcl.eth/comms"
        );
        assert_eq!(
            local_about(&down, None).comms.adapter,
            "fixed-adapter:offline:offline",
            "an unreachable SFU falls back to single-player"
        );
    }

    #[test]
    fn a_world_without_scenes_is_404_and_a_blocked_world_401() {
        let cfg = super::super::Settings::for_tests("https://worlds.example.org");
        let err = build_about(
            &cfg,
            "none.dcl.eth",
            WorldAbout {
                world: None,
                scenes: vec![],
            },
        )
        .unwrap_err();
        assert!(matches!(err, AppError::NotFound(m) if m.contains("no scenes deployed")));

        let blocked = WorldAbout {
            world: Some(WorldRecord {
                name: "b.dcl.eth".into(),
                blocked_since: Some(
                    chrono::DateTime::from_timestamp(1_700_000_000, 0)
                        .unwrap()
                        .naive_utc(),
                ),
                spawn_coordinates: None,
                skybox_time: None,
                single_player: false,
                realm_name_override: None,
            }),
            scenes: vec![scene(Some("https://up.example"))],
        };
        let err = build_about(&cfg, "b.dcl.eth", blocked).unwrap_err();
        assert!(
            matches!(err, AppError::Unauthorized(m) if m.contains("blocked since 2023-11-14T22:13:20.000Z"))
        );
    }
}
