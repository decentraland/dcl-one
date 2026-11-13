// Scene map pins: a scene marks a parcel on the player's map with a title, a description and
// optionally its own icon. The pins of every running scene are streamed to the system scene,
// which draws them; a pin goes when its entity or its scene does. What a scene can put there is
// bounded: so many pins, so much text, and the list goes out a few times a second at most.
use std::time::Duration;

use bevy::{platform::collections::HashMap, prelude::*};

use common::rpc::RpcStreamSender;
use dcl_component::{
    proto_components::{common::texture_union::Tex, sdk::components::PbMapPin},
    SceneComponentId,
};
use dcl_types::interface::ComponentPosition;
use ipfs::IpfsAssetServer;
use system_bridge::{MapPinInfo, SystemApi};
use url::Url;

use crate::{RendererSceneContext, SceneEntity};

use super::AddCrdtInterfaceExt;

/// The most pins one scene shows; those of its entities past it are left out.
const MAX_PINS_PER_SCENE: usize = 64;
const MAX_TITLE_CHARS: usize = 256;
const MAX_DESCRIPTION_CHARS: usize = 1024;
const SEND_INTERVAL: Duration = Duration::from_millis(250);

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
            Some(Tex::Texture(texture)) if !texture.src.is_empty() => icon_source(texture.src),
            _ => None,
        };
        Self {
            // a pin between parcels belongs to the nearer one; halfway, to the even one
            parcel: IVec2::new(
                position.x.round_ties_even() as i32,
                position.y.round_ties_even() as i32,
            ),
            title: clip(value.title, MAX_TITLE_CHARS),
            description: clip(value.description, MAX_DESCRIPTION_CHARS),
            icon,
        }
    }
}

/// A url is kept only over https (plain http would be mixed content in the page that draws it),
/// as the parser writes it, so the page can quote it. Anything else is a file of the scene.
fn icon_source(src: String) -> Option<String> {
    match Url::parse(&src) {
        Ok(url) => (url.scheme() == "https").then(|| url.to_string()),
        Err(_) => Some(src),
    }
}

fn clip(mut text: String, max: usize) -> String {
    if let Some((end, _)) = text.char_indices().nth(max) {
        text.truncate(end);
    }
    text
}

/// Each scene's first `MAX_PINS_PER_SCENE` pins by entity, in a stable order.
fn bounded(mut pins: Vec<(String, u32, MapPinInfo)>) -> Vec<MapPinInfo> {
    pins.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    let mut counts = HashMap::<String, usize>::default();
    pins.into_iter()
        .filter_map(|(scene, _, pin)| {
            let count = counts.entry(scene).or_default();
            *count += 1;
            (*count <= MAX_PINS_PER_SCENE).then_some(pin)
        })
        .collect()
}

#[derive(Default)]
struct Sending {
    dirty: bool,
    since_sent: Option<Duration>,
}

#[allow(clippy::too_many_arguments)]
fn stream_map_pins(
    mut requests: EventReader<SystemApi>,
    mut senders: Local<Vec<RpcStreamSender<Vec<MapPinInfo>>>>,
    mut sent: Local<Vec<MapPinInfo>>,
    mut sending: Local<Sending>,
    time: Res<Time<Real>>,
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

    sending.dirty |= !changed.is_empty() || removed.read().count() > 0;
    sending.since_sent = sending.since_sent.map(|since| since + time.delta());
    if sending.dirty
        && sending
            .since_sent
            .is_none_or(|since| since >= SEND_INTERVAL)
    {
        sending.dirty = false;
        sending.since_sent = Some(Duration::ZERO);
        let current = pins
            .iter()
            .filter_map(|(pin, entity)| {
                let scene = scenes.get(entity.root).ok()?;
                // a url was kept only over https; the rest are the scene's files
                let icon = pin.icon.as_ref().and_then(|src| {
                    if Url::parse(src).is_ok() {
                        Some(src.clone())
                    } else {
                        ipfas.ipfs().content_url(src, &scene.hash)
                    }
                });
                Some((
                    scene.hash.to_string(),
                    u32::from(entity.id.generation) << 16 | u32::from(entity.id.id),
                    MapPinInfo {
                        id: format!("{}:{}", scene.hash, entity.id),
                        x: pin.parcel.x,
                        y: pin.parcel.y,
                        title: pin.title.clone(),
                        description: pin.description.clone(),
                        icon,
                    },
                ))
            })
            .collect::<Vec<_>>();
        let current = bounded(current);
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

        let icon = |src: &str| {
            MapPin::from(PbMapPin {
                texture: Some(TextureUnion {
                    tex: Some(Tex::Texture(Texture {
                        src: src.into(),
                        ..Default::default()
                    })),
                }),
                ..Default::default()
            })
            .icon
        };
        assert_eq!(
            icon("https://example.com/a.png").as_deref(),
            Some("https://example.com/a.png")
        );
        assert_eq!(icon("http://example.com/a.png"), None);
        assert_eq!(icon("HTTP://example.com/a.png"), None);
        assert_eq!(icon("data:image/png;base64,AAAA"), None);
        assert_eq!(icon("javascript:alert(1)"), None);
        assert_eq!(
            icon("HTTPS://example.com/a.png").as_deref(),
            Some("https://example.com/a.png")
        );
        // a quote cannot close the url() the page writes the icon into
        assert_eq!(
            icon(r#"https://example.com/a.png"),url("https://example.com/b.png"#).as_deref(),
            Some("https://example.com/a.png%22),url(%22https://example.com/b.png")
        );
    }

    #[test]
    fn a_pin_keeps_so_much_text() {
        let pin = MapPin::from(PbMapPin {
            title: "é".repeat(MAX_TITLE_CHARS + 10),
            description: "d".repeat(MAX_DESCRIPTION_CHARS + 10),
            ..Default::default()
        });
        assert_eq!(pin.title.chars().count(), MAX_TITLE_CHARS);
        assert_eq!(pin.description.len(), MAX_DESCRIPTION_CHARS);
        assert_eq!(clip("short".into(), MAX_TITLE_CHARS), "short");
    }

    #[test]
    fn a_scene_shows_so_many_pins() {
        let pin = |scene: &str, entity: u32| {
            (
                scene.to_owned(),
                entity,
                MapPinInfo {
                    id: format!("{scene}:{entity}"),
                    x: 0,
                    y: 0,
                    title: String::new(),
                    description: String::new(),
                    icon: None,
                },
            )
        };
        let mut pins = (0..MAX_PINS_PER_SCENE as u32 + 5)
            .rev()
            .map(|entity| pin("busy", entity))
            .collect::<Vec<_>>();
        pins.push(pin("quiet", 900));
        let shown = bounded(pins);
        assert_eq!(shown.len(), MAX_PINS_PER_SCENE + 1);
        // the lowest entities stay, whatever order they came in, and another scene is untouched
        assert_eq!(shown[0].id, "busy:0");
        assert_eq!(
            shown[MAX_PINS_PER_SCENE - 1].id,
            format!("busy:{}", MAX_PINS_PER_SCENE - 1)
        );
        assert_eq!(shown[MAX_PINS_PER_SCENE].id, "quiet:900");
    }
}
