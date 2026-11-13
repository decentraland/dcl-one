// Scene map pins: a scene marks a parcel on the player's map with a title, a description and
// optionally its own icon. The pins of every running scene are streamed to the system scene,
// which draws them; a pin goes when its entity or its scene does.
use bevy::prelude::*;

use common::rpc::RpcStreamSender;
use dcl_component::{
    proto_components::{common::texture_union::Tex, sdk::components::PbMapPin},
    SceneComponentId,
};
use dcl_types::interface::ComponentPosition;
use ipfs::IpfsAssetServer;
use system_bridge::{MapPinInfo, SystemApi};

use crate::{RendererSceneContext, SceneEntity};

use super::AddCrdtInterfaceExt;

pub struct MapPinPlugin;

impl Plugin for MapPinPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbMapPin, MapPin>(
            SceneComponentId::MAP_PIN,
            ComponentPosition::EntityOnly,
        );
        app.add_systems(Update, stream_map_pins);
    }
}

#[derive(Component, Clone, PartialEq, Debug)]
pub struct MapPin {
    pub parcel: IVec2,
    pub title: String,
    pub description: String,
    /// The icon's source as the scene gave it: a file of the scene, or a url.
    pub icon: Option<String>,
}

impl From<PbMapPin> for MapPin {
    fn from(value: PbMapPin) -> Self {
        let position = value.position.unwrap_or_default();
        let icon = match value.texture.and_then(|texture| texture.tex) {
            Some(Tex::Texture(texture)) if !texture.src.is_empty() => Some(texture.src),
            _ => None,
        };
        Self {
            // a pin between parcels belongs to the nearer one; halfway, to the even one
            parcel: IVec2::new(
                position.x.round_ties_even() as i32,
                position.y.round_ties_even() as i32,
            ),
            title: value.title,
            description: value.description,
            icon,
        }
    }
}

fn stream_map_pins(
    mut requests: EventReader<SystemApi>,
    mut senders: Local<Vec<RpcStreamSender<Vec<MapPinInfo>>>>,
    mut sent: Local<Vec<MapPinInfo>>,
    pins: Query<(&MapPin, &SceneEntity)>,
    changed: Query<(), Changed<MapPin>>,
    mut removed: RemovedComponents<MapPin>,
    scenes: Query<&RendererSceneContext>,
    ipfas: IpfsAssetServer,
) {
    let new_senders = requests
        .read()
        .filter_map(|ev| match ev {
            SystemApi::GetMapPinsStream(sender) => Some(sender.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    senders.retain(|s| !s.is_closed());

    let pins_changed = !changed.is_empty() || removed.read().count() > 0;
    if pins_changed {
        let mut current = pins
            .iter()
            .filter_map(|(pin, entity)| {
                let scene = scenes.get(entity.root).ok()?;
                let icon = pin.icon.as_ref().and_then(|src| {
                    if src.starts_with("https://") || src.starts_with("http://") {
                        Some(src.clone())
                    } else {
                        ipfas.ipfs().content_url(src, &scene.hash)
                    }
                });
                Some(MapPinInfo {
                    id: format!("{}:{}", scene.hash, entity.id),
                    x: pin.parcel.x,
                    y: pin.parcel.y,
                    title: pin.title.clone(),
                    description: pin.description.clone(),
                    icon,
                })
            })
            .collect::<Vec<_>>();
        current.sort_by(|a, b| a.id.cmp(&b.id));
        if current != *sent {
            *sent = current;
            for sender in senders.iter() {
                let _ = sender.send(sent.clone());
            }
        }
    }

    // a new subscriber starts from the pins as they stand
    for sender in new_senders {
        let _ = sender.send(sent.clone());
        senders.push(sender);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_component::proto_components::common::{Texture, TextureUnion, Vector2};

    #[test]
    fn a_pin_stands_on_the_parcel_nearest_its_position() {
        let pin = MapPin::from(PbMapPin {
            position: Some(Vector2 { x: 10.4, y: -3.6 }),
            title: "Quest".into(),
            description: "Find the key".into(),
            ..Default::default()
        });
        assert_eq!(pin.parcel, IVec2::new(10, -4));
        assert_eq!(pin.title, "Quest");
        assert_eq!(pin.icon, None);

        // halfway goes to the even parcel
        let halfway = MapPin::from(PbMapPin {
            position: Some(Vector2 { x: 2.5, y: 3.5 }),
            ..Default::default()
        });
        assert_eq!(halfway.parcel, IVec2::new(2, 4));
    }

    #[test]
    fn a_pin_keeps_the_source_of_a_plain_texture_only() {
        let with_icon = MapPin::from(PbMapPin {
            texture: Some(TextureUnion {
                tex: Some(Tex::Texture(Texture {
                    src: "images/pin.png".into(),
                    ..Default::default()
                })),
            }),
            ..Default::default()
        });
        assert_eq!(with_icon.icon.as_deref(), Some("images/pin.png"));

        let empty = MapPin::from(PbMapPin {
            texture: Some(TextureUnion { tex: None }),
            ..Default::default()
        });
        assert_eq!(empty.icon, None);
    }
}
