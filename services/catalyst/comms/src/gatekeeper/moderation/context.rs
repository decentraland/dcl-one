//! A moderation write has one authority: its verified signed deployment.
//! Unsigned body/query selectors can confirm that authority, never supply it.
use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::gatekeeper::CommsState;
use crate::gatekeeper::admission::scene::{PlaceInfo, PlaceLookup};
use crate::gatekeeper::errors::{ApiError, service_unavailable};
use crate::rooms::{ResolveError, RoomRequest};
use crate::world_identity::{Parcel, RealmId, SceneId, WorldName, signed_realm_name};

fn invalid() -> ApiError {
    ApiError::bad_request("Invalid signed scene context")
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str, ApiError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(invalid)
}

fn resolution_error(error: ResolveError) -> ApiError {
    match error {
        ResolveError::Db(error) => {
            tracing::warn!(%error, "moderation scene resolver unavailable");
            service_unavailable("Scene identity is unavailable")
        }
        _ => ApiError::bad_request("Signed scene is not currently deployed at this place"),
    }
}

pub(in crate::gatekeeper) struct Context<'a> {
    pub place: PlaceLookup<'a>,
    /// The current scene's room at the bound parcel.
    pub room: String,
    /// The bound deployment's world and parcels.
    pub footprint: PlaceInfo,
    /// Where this scene's bans live; stable across a world scene's redeploys.
    pub ban_key: crate::access::SceneBanKey,
    pub rooms: Vec<String>,
    pub place_title: String,
    /// The world and current scene a world-scene ban also reaches replaced rooms of.
    pub world_scene: Option<(WorldName, SceneId)>,
}

impl<'a> Context<'a> {
    /// The realm the signed metadata names ([`signed_realm_name`]): a world by
    /// its realm name or by the realm URL or worlds host signed beside it, else
    /// a shared realm. `node_realm` is this node's `REALM_NAME`.
    pub fn signed_realm(metadata: &Value, node_realm: &str) -> Result<RealmId, ApiError> {
        let nested = match metadata.get("realm") {
            Some(realm) if realm.is_object() => Some(required(realm, "serverName")?),
            Some(_) => return Err(invalid()),
            None => None,
        };
        let flat = match metadata.get("realmName") {
            Some(Value::String(name)) if !name.is_empty() => Some(name.as_str()),
            Some(_) => return Err(invalid()),
            None => None,
        };
        if let (Some(nested), Some(flat)) = (nested, flat)
            && !nested.eq_ignore_ascii_case(flat)
        {
            return Err(invalid());
        }
        let realm_name = nested.or(flat).ok_or_else(invalid)?;
        let parsed_name = WorldName::parse(realm_name).map_err(|_| invalid())?;
        if parsed_name.display() != realm_name || realm_name.contains(':') {
            return Err(invalid());
        }
        let hostname = match metadata.pointer("/realm/hostname") {
            Some(Value::String(hostname)) => {
                // Explorer metadata carries a bare hostname; existing target
                // callers also use an absolute realm URL.
                let absolute = if hostname.contains("://") {
                    hostname.clone()
                } else {
                    format!("https://{hostname}")
                };
                let url = url::Url::parse(&absolute).map_err(|_| invalid())?;
                if !matches!(url.scheme(), "http" | "https")
                    || url.host_str().is_none()
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || url.query().is_some()
                    || url.fragment().is_some()
                {
                    return Err(invalid());
                }
                Some(hostname.as_str())
            }
            Some(_) => return Err(invalid()),
            None => None,
        };
        RealmId::parse(&signed_realm_name(realm_name, hostname, node_realm)).map_err(|_| invalid())
    }

    pub async fn resolve(state: &'a CommsState, metadata: &Value) -> Result<Self, ApiError> {
        let realm = Self::signed_realm(metadata, &state.settings.realm_name)?;
        let raw_scene = required(metadata, "sceneId")?;
        let scene = SceneId::parse(raw_scene).map_err(|_| invalid())?;
        if scene.as_str() != raw_scene || (!realm.is_world() && raw_scene.contains(':')) {
            return Err(invalid());
        }
        if scene.is_world_shaped()
            && !realm
                .world()
                .is_some_and(|world| raw_scene.eq_ignore_ascii_case(world.display()))
        {
            return Err(invalid());
        }
        let supplied_parcel = match metadata.get("parcel") {
            Some(Value::String(raw)) => {
                let parcel = Parcel::parse(raw).map_err(|_| invalid())?;
                if parcel.canonical() != *raw {
                    return Err(invalid());
                }
                Some(parcel)
            }
            Some(_) => return Err(invalid()),
            None => None,
        };
        let request = match (&realm, supplied_parcel) {
            (RealmId::World(_), Some(parcel)) if scene.is_world_shaped() => RoomRequest::Parcel {
                realm: &realm,
                parcel,
            },
            (RealmId::World(world), _) if !scene.is_world_shaped() => RoomRequest::WorldScene {
                world,
                scene: &scene,
            },
            _ => RoomRequest::Adapter {
                realm: &realm,
                scene: &scene,
            },
        };
        let resolved = state
            .scenes
            .resolve(request)
            .await
            .map_err(resolution_error)?;
        let deployment = resolved.deployment.as_ref().ok_or_else(invalid)?;
        if !scene.is_world_shaped() && deployment.scene_id != scene {
            return Err(invalid());
        }
        let parcel = supplied_parcel.unwrap_or(deployment.base);
        if !deployment.parcels.contains(&parcel) {
            return Err(invalid());
        }
        // Adapter can return an old Genesis deployment; a current parcel
        // lookup binds the signed id to today's placement even without parcel.
        let current = state
            .scenes
            .resolve(RoomRequest::Parcel {
                realm: &realm,
                parcel,
            })
            .await
            .map_err(resolution_error)?;
        let current_deployment = current.deployment.as_ref().ok_or_else(invalid)?;
        if resolved.realm != realm
            || current.scene_id.as_ref() != Some(&deployment.scene_id)
            || current_deployment.scene_id != deployment.scene_id
            || current.realm != resolved.realm
        {
            return Err(invalid());
        }
        let info = PlaceInfo {
            world: realm.is_world(),
            world_name: realm.world().map(|world| world.key().to_owned()),
            positions: current_deployment
                .parcels
                .iter()
                .map(Parcel::canonical)
                .collect(),
            base_position: Some(current_deployment.base.canonical()),
        };
        let ban_key = current.ban_key().ok_or_else(invalid)?;
        let mut rooms = Vec::new();
        if state.livekit.configured {
            if let Some(world) = realm.world() {
                rooms.push(crate::rooms::worlds_service_world_room(world));
            }
            rooms.push(current.room.clone());
        }
        let world_scene = realm
            .world()
            .map(|world| (world.clone(), deployment.scene_id.clone()));
        Ok(Self {
            room: current.room,
            footprint: info.clone(),
            world_scene,
            place: PlaceLookup::resolved(
                state,
                deployment.scene_id.as_str(),
                info,
                &current_deployment.place_key(),
            ),
            ban_key,
            rooms,
            place_title: current_deployment
                .title
                .as_deref()
                .map(crate::shared::markup::strip_all_markup)
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| format!("Place at {}", current_deployment.base.canonical())),
        })
    }

    /// The live rooms of scenes this world scene replaced (still mintable a
    /// bounded window) that its ban reaches. Refuse an incomplete inventory before
    /// accepting the durable ban/removal write.
    pub async fn replaced_scene_rooms(&self, state: &CommsState) -> Result<Vec<String>, ApiError> {
        let Some((world, current)) = self.world_scene.as_ref() else {
            return Ok(Vec::new());
        };
        if !state.livekit.configured {
            return Ok(Vec::new());
        }
        let rooms = match crate::gatekeeper::admission::scene::live_world_rooms(state, world).await
        {
            Ok(rooms) => rooms,
            Err(error) => {
                tracing::warn!(%error, world = %world, "replaced scene rooms not listed; refusing incomplete moderation inventory");
                return Err(service_unavailable(
                    "Scene room inventory unavailable; retry the moderation request.",
                ));
            }
        };
        let mut out = Vec::new();
        for room in rooms {
            let Some((_, scene)) = crate::rooms::decode_world_scene_room(&room) else {
                continue;
            };
            if &scene == current || self.rooms.contains(&room) {
                continue;
            }
            match state.scenes.replaced_world_scene(world, &scene).await {
                Ok(Some(replaced)) if replaced.ban_keys(world).contains(&self.ban_key) => {
                    out.push(room)
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%error, room = %room, "replaced scene unresolved; refusing incomplete moderation inventory");
                    return Err(service_unavailable(
                        "Scene room inventory unavailable; retry the moderation request.",
                    ));
                }
            }
        }
        Ok(out)
    }

    pub fn check_selectors(&self, input: &Input) -> Result<(), ApiError> {
        for selector in [
            input.body.get("place_id").and_then(Value::as_str),
            input.query.get("place_id").map(String::as_str),
        ]
        .into_iter()
        .flatten()
        {
            if selector != self.place.place_id() {
                return Err(ApiError::bad_request(
                    "place_id does not match the signed scene",
                ));
            }
        }
        Ok(())
    }
}

pub(in crate::gatekeeper) struct Input {
    body: Map<String, Value>,
    query: BTreeMap<String, String>,
}

impl Input {
    pub fn parse(
        bytes: &[u8],
        query: BTreeMap<String, String>,
        address_key: &str,
        name_key: Option<&str>,
        allow_query_target: bool,
    ) -> Result<Self, ApiError> {
        let body = if bytes.is_empty() && allow_query_target {
            Map::new()
        } else {
            serde_json::from_slice::<Value>(bytes)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .ok_or_else(|| ApiError::bad_request("Invalid JSON body"))?
        };
        for (key, value) in &body {
            if !(key == "place_id" || key == address_key || name_key == Some(key.as_str()))
                || !value.is_string()
            {
                return Err(ApiError::bad_request("Invalid moderation request field"));
            }
        }
        for key in query.keys() {
            if !(key == "place_id" || (allow_query_target && key == address_key)) {
                return Err(ApiError::bad_request("Invalid moderation query field"));
            }
        }
        if let (Some(body), Some(query)) = (
            body.get(address_key).and_then(Value::as_str),
            query.get(address_key),
        ) && !body.eq_ignore_ascii_case(query)
        {
            return Err(ApiError::bad_request("Conflicting moderation targets"));
        }
        if name_key.is_some_and(|name| body.contains_key(name))
            && !body.contains_key(address_key)
            && query.contains_key(address_key)
        {
            return Err(ApiError::bad_request("Conflicting moderation targets"));
        }
        Ok(Self { body, query })
    }

    pub fn address<'a>(&'a self, key: &str) -> Option<&'a str> {
        self.body
            .get(key)
            .and_then(Value::as_str)
            .or_else(|| self.query.get(key).map(String::as_str))
    }

    pub fn name<'a>(&'a self, key: &str) -> Option<&'a str> {
        self.body.get(key).and_then(Value::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gatekeeper::moderation::test_support::{StubBanStatus, state};
    use crate::interfaces::test_support::{StubPolicy, StubScenes};
    use crate::rooms::{ActiveDeployment, ResolvedRoom};
    use crate::world_identity::SceneNamespace;
    use std::sync::Arc;

    #[tokio::test]
    async fn title_is_exact_deployment_display_data_with_resolved_base_fallback() {
        for (title, expected) in [
            (Some("Exact Scene B"), "Exact Scene B"),
            (None, "Place at 1,1"),
            (Some(""), "Place at 1,1"),
            (Some("<link=\"smb://h/s\">Rules</link>"), "Rules"),
            (Some("<size=1></size>"), "Place at 1,1"),
        ] {
            let world = WorldName::parse("title.dcl.eth").unwrap();
            let realm = RealmId::World(world.clone());
            let scene = SceneId::parse("SceneB").unwrap();
            let base = Parcel::parse("1,1").unwrap();
            let resolved = ResolvedRoom {
                room: crate::rooms::worlds_service_scene_room(&world, &scene),
                realm: realm.clone(),
                scene_id: Some(scene.clone()),
                deployment: Some(ActiveDeployment {
                    scene_id: scene,
                    title: title.map(String::from),
                    parcels: [base].into_iter().collect(),
                    base,
                    namespace: SceneNamespace::new(&realm, base, None),
                    local: true,
                }),
            };
            let mut state = state(StubPolicy::default(), StubBanStatus::default());
            state.scenes = Arc::new(StubScenes {
                room: Ok(resolved),
                scene: Ok(None),
            });
            let context = Context::resolve(
                &state,
                &serde_json::json!({"realmName":"title.dcl.eth","sceneId":"SceneB","parcel":"1,1"}),
            )
            .await
            .unwrap();
            assert_eq!(context.place.place_id(), "SceneB");
            assert_eq!(context.place_title, expected);
        }
    }

    /// A world published here advertises `title` (no `.dcl.eth`) or an owner's
    /// override; the signed realm URL `<host>/world/<name>` names the world, as
    /// does `title` beside the worlds host alone (the Unity client).
    #[tokio::test]
    async fn a_stripped_realm_name_resolves_in_the_world_its_signed_realm_url_names() {
        let world = WorldName::parse("title.dcl.eth").unwrap();
        let realm = RealmId::World(world.clone());
        let scene = SceneId::parse("SceneB").unwrap();
        let base = Parcel::parse("1,1").unwrap();
        let resolved = ResolvedRoom {
            room: crate::rooms::worlds_service_scene_room(&world, &scene),
            realm: realm.clone(),
            scene_id: Some(scene.clone()),
            deployment: Some(ActiveDeployment {
                scene_id: scene,
                title: None,
                parcels: [base].into_iter().collect(),
                base,
                namespace: SceneNamespace::new(&realm, base, None),
                local: true,
            }),
        };
        let mut state = state(StubPolicy::default(), StubBanStatus::default());
        state.scenes = Arc::new(StubScenes {
            room: Ok(resolved),
            scene: Ok(None),
        });
        for (server_name, hostname) in [
            ("title", "node.example/world/title.dcl.eth"),
            (
                "title",
                "https://node.example/worlds-content-server/world/title.dcl.eth/",
            ),
            ("Custom-Realm", "node.example/world/title.dcl.eth"),
            ("title", "worlds-content-server.node.example"),
        ] {
            let context = Context::resolve(
                &state,
                &serde_json::json!({"realm":{"serverName":server_name,"hostname":hostname},"sceneId":"SceneB","parcel":"1,1"}),
            )
            .await
            .unwrap_or_else(|e| panic!("{server_name} at {hostname}: {}", e.message));
            assert_eq!(context.place.place_id(), "SceneB");
            assert_eq!(
                context.footprint.world_name.as_deref(),
                Some("title.dcl.eth")
            );
        }
        // Without a world in the realm URL the stripped name is a shared realm.
        let err = Context::resolve(
            &state,
            &serde_json::json!({"realm":{"serverName":"title","hostname":"node.example"},"sceneId":"SceneB","parcel":"1,1"}),
        )
        .await
        .map(|_| ())
        .unwrap_err();
        assert_eq!(err.code, 400);
    }
}
