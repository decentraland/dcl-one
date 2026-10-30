//! A moderation write has one authority: its verified signed deployment.
//! Unsigned body/query selectors can confirm that authority, never supply it.
use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::gatekeeper::CommsState;
use crate::gatekeeper::admission::scene::{PlaceInfo, PlaceLookup};
use crate::gatekeeper::errors::{ApiError, service_unavailable};
use crate::rooms::{ResolveError, RoomRequest};
use crate::world_identity::{Parcel, RealmId, SceneId, WorldName, is_world_realm_name};

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

pub(super) struct Context<'a> {
    pub place: PlaceLookup<'a>,
    pub rooms: Vec<String>,
    pub place_title: String,
}

impl<'a> Context<'a> {
    pub async fn resolve(state: &'a CommsState, metadata: &Value) -> Result<Self, ApiError> {
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
        let hostname_world = match metadata.pointer("/realm/hostname") {
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
                url.host_str()
                    .is_some_and(|host| host.contains("worlds-content-server"))
            }
            Some(_) => return Err(invalid()),
            None => false,
        };
        let realm = if hostname_world || is_world_realm_name(realm_name) {
            RealmId::World(parsed_name)
        } else {
            RealmId::Shared(realm_name.to_owned())
        };
        let raw_scene = required(metadata, "sceneId")?;
        let scene = SceneId::parse(raw_scene).map_err(|_| invalid())?;
        if scene.as_str() != raw_scene || (!realm.is_world() && raw_scene.contains(':')) {
            return Err(invalid());
        }
        if scene.is_world_shaped()
            && (!realm.is_world() || !raw_scene.eq_ignore_ascii_case(realm_name))
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
        let request = match &realm {
            RealmId::World(_) if scene.is_world_shaped() && supplied_parcel.is_some() => {
                RoomRequest::Parcel {
                    realm: &realm,
                    parcel: supplied_parcel.unwrap(),
                }
            }
            RealmId::World(world) if !scene.is_world_shaped() => RoomRequest::WorldScene {
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
        let mut rooms = Vec::new();
        if state.livekit.configured {
            if let Some(world) = realm.world() {
                rooms.push(crate::rooms::worlds_service_world_room(world));
            }
            rooms.push(current.room);
        }
        Ok(Self {
            place: PlaceLookup::resolved(state, deployment.scene_id.as_str(), info),
            rooms,
            place_title: current_deployment
                .title
                .as_ref()
                .filter(|title| !title.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("Place at {}", current_deployment.base.canonical())),
        })
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
}

pub(super) struct Input {
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
